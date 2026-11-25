// Showcase — Overlays & Surfaces tab.
//
// Dedicated home for Halcyon's transient-surface family. Each section
// exercises one surface type behind its own trigger. Built up surface by
// surface; this revision establishes the tab shell and the Tooltips section.
//
// Tooltips — SPEC-HAL-226 / 226A. A Display-only target registers a tooltip
// trigger for its painted rect when enabled; the framework
// TooltipManager (AppRoot) owns timing, placement, and dismissal. The target
// accepts no input (hit_testable = false) — tooltip hover is the only legal
// interaction for a Display element.

use halcyon::core::app::{
    ChildWindowAction, ChildWindowActionRegion, ChildWindowPaintContext, ChildWindowSceneState,
    ScreenContext, ScreenFrameContext,
};
use halcyon::core::event::{Event, EventPhase, FusedInputEvent, PointerButton};
use halcyon::core::layout::{Constraints, EdgeInsets, FlexParams, LayoutResult};
use halcyon::core::math::{RectF, Size, snap_to_physical};
use halcyon::core::paint::{Color, ZIndex};
use halcyon::core::text_selection::TextSelectionPolicy;
use halcyon::core::widget::{EventContext, LayoutContext, PaintContext, Widget, WidgetDowncast};
use halcyon::core::widget_tree::NodeId;
use halcyon::model::context_menu::{Anchor, context_menu_handle};
use halcyon::model::dialog::DialogHandle;
use halcyon::model::menu_item::MenuItem;
use halcyon::model::toast::ToastAction;
use halcyon::model::tooltip::{TooltipPayload, TooltipPlacement};
use halcyon::prelude::*;
use halcyon::utils::id::WidgetInternalId;
use halcyon::widgets::auxiliary_text::AuxiliaryText;
use halcyon::widgets::context_menu::ContextMenuScreen;
use halcyon::widgets::dialog::{Dialog, DialogButton, DialogConfig};
use halcyon::widgets::divider::Divider;
use halcyon::widgets::section_header::SectionHeader;
use halcyon::widgets::{
    KeyValueItem, KeyValueStrip, ReadoutCell, ReadoutColumn, ReadoutTable, StatusGrid,
    StatusGridCell, StatusGridMetric, StatusGridWideRow,
};
use std::rc::Rc;

use super::*;

const CHILD_WINDOW_TEXT_LAYER: ZIndex = 7;

/// Display-only tooltip target. Renders as a flat control cell and registers a
/// tooltip trigger over its painted rect (SPEC-HAL-226A). No input handling.
struct TooltipTarget {
    id: WidgetInternalId,
    label: String,
    tip: String,
    placement: TooltipPlacement,
    tooltip_enabled: bool,
}

impl TooltipTarget {
    fn new(label: &str, tip: &str, placement: TooltipPlacement, tooltip_enabled: bool) -> Self {
        Self {
            id: WidgetInternalId::new(),
            label: label.to_string(),
            tip: tip.to_string(),
            placement,
            tooltip_enabled,
        }
    }
}

impl WidgetDowncast for TooltipTarget {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for TooltipTarget {
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        let font_size = cx.theme.typography.font_size;
        let run = cx.text_engine.shape(&self.label, font_size);
        let w = (run.width + 24.0).min(constraints.max_width);
        let h = (run.height + 10.0).min(constraints.max_height);
        LayoutResult::fit(Size::new(w, h))
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        cx.scene
            .push_rect(rect, cx.theme.colors.surfaces.level2, 1, None);
        cx.scene.push_border_rect(
            rect,
            cx.theme.colors.borders.default,
            cx.theme.borders.b1,
            2,
            None,
        );

        let font_size = cx.theme.typography.font_size;
        let run = cx.text_engine.shape(&self.label, font_size);
        let tx = rect.x + (rect.width - run.width) / 2.0;
        let ty = rect.y + (rect.height - run.height) / 2.0;
        cx.scene.push_text_with_backgrounds(
            &run,
            tx,
            ty,
            cx.theme.colors.typography.primary,
            Vec::new(),
            3,
            None,
        );

        if self.tooltip_enabled {
            cx.register_tooltip_at(rect, TooltipPayload::Text(self.tip.clone()), self.placement);
        }
    }

    fn handle_event(&mut self, _cx: &mut EventContext, _event: &Event, _phase: EventPhase) -> bool {
        false
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn hit_testable(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use halcyon::core::app::{AppContext, EngineTicker, TooltipRenderState, build_scene_with_appearance};
    use halcyon::core::event::{InteractionDomains, WindowId};
    use halcyon::core::layout_transition::ActiveLayoutTransitions;
    use halcyon::core::math::Point;
    use halcyon::core::scene::SceneNode;
    use halcyon::core::transition::TransitionLayer;
    use halcyon::core::widget_tree::WidgetTree;
    use halcyon::model::tooltip::{TooltipManager, TooltipTiming};
    use halcyon::platform::font_manager_text_engine::FontManagerTextEngine;

    fn leaves(nodes: &[SceneNode]) -> impl Iterator<Item = &SceneNode> {
        nodes.iter().flat_map(|node| -> Box<dyn Iterator<Item = &SceneNode>> {
            match node {
                SceneNode::RetainedFragment(n) => Box::new(leaves(&n.children)),
                SceneNode::Clip(n) => Box::new(leaves(&n.children)),
                SceneNode::Layer(n) => Box::new(leaves(&n.children)),
                SceneNode::Scroll(n) => Box::new(leaves(&n.children)),
                _ => Box::new(std::iter::once(node)),
            }
        })
    }

    #[test]
    fn showcase_tooltip_targets_are_outlined_and_disabled_dwell_paints_no_tooltip() {
        let theme = showcase_app_config(std::iter::empty()).theme;
        let mut app = AppContext::new(theme.clone());
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(Flex::column()), None, 0);
        ShowcaseScreen::new().build_overlays(
            &mut ScreenFrameContext {
                app: &mut app,
                tree: &mut tree,
                parent_node: root,
            },
            root,
        );

        let mut targets = std::collections::HashMap::new();
        let mut pending = vec![root];
        while let Some(id) = pending.pop() {
            let widget = tree.node(id).unwrap().widget();
            if let Some(target) = widget.as_any().downcast_ref::<TooltipTarget>() {
                assert!(!widget.hit_testable());
                assert!(targets.insert(target.label.clone(), id).is_none());
            }
            pending.extend(tree.children(id));
        }
        assert_eq!(targets.len(), 4);

        let mut ticker = EngineTicker::new(Instant::now());
        let mut layer = TransitionLayer::default();
        let mut transitions = ActiveLayoutTransitions::default();
        let mut text = FontManagerTextEngine::new();
        let icons = halcyon::core::icons::builtin_registry();
        let domains = InteractionDomains::new();
        let mut draw = |tree: &mut WidgetTree, tooltip: Option<&TooltipRenderState>| {
            ticker.begin_frame();
            let mut declarations = Vec::new();
            let scene = build_scene_with_appearance(
                tree, root, Constraints::tight(Size::new(1000.0, 900.0)), &theme,
                &mut text, &icons, 0, 0.0, &mut layer, &mut transitions, None, 1.0,
                None, None, false, &mut declarations, tooltip, &domains, WindowId::PRIMARY,
            );
            (scene, declarations)
        };
        let (initial, declarations) = draw(&mut tree, None);
        let mut manager = TooltipManager::new(TooltipTiming {
            show_delay_ms: theme.tooltip.show_delay_ms,
            hide_delay_ms: theme.tooltip.hide_delay_ms,
            reshow_window_ms: theme.tooltip.reshow_window_ms,
        });
        manager.triggers_mut().extend(declarations.into_iter().filter(|trigger| {
            targets.values().any(|id| tree.widget_id_for_node(*id) == Some(trigger.widget_id))
        }));
        assert_eq!(manager.triggers_mut().len(), 3);

        for (label, tip, placement) in [
            ("North", Some("Above target"), TooltipPlacement::Above),
            ("East", Some("Right of target"), TooltipPlacement::Right),
            ("Cursor", Some("Auto placement"), TooltipPlacement::Auto),
            ("Disabled", None, TooltipPlacement::Auto),
        ] {
            let id = targets[label];
            let face = tree.get_absolute_paint_rect(id).unwrap();
            assert!(leaves(&initial.root).any(|node| matches!(node, SceneNode::Rect(fill)
                if fill.rect == face && fill.z_index == 1 && fill.color == theme.colors.surfaces.level2.into())));
            assert!(leaves(&initial.root).any(|node| matches!(node, SceneNode::BorderRect(outline)
                if outline.rect == face && outline.z_index == 2
                    && outline.border_color == theme.colors.borders.default.into()
                    && outline.border_width == theme.borders.b1)));
            assert!(leaves(&initial.root).any(|node| matches!(node, SceneNode::Text(text)
                if text.text == label && text.z_index == 3)));

            let center = Point::new(face.x + face.width * 0.5, face.y + face.height * 0.5);
            let declaration = manager.find_trigger_at(center.x, center.y);
            if tip.is_some() {
                let declaration = declaration.unwrap();
                assert_eq!(Some(declaration.widget_id), tree.widget_id_for_node(id));
                assert_eq!(declaration.placement, placement);
            } else {
                assert!(declaration.is_none());
            }

            manager.update_from_pointer(center.x, center.y, 1_000);
            manager.tick(theme.tooltip.show_delay_ms as f32 / 1_000.0 + 0.001, 1_501);
            let tooltip = manager.content().cloned().map(|content| TooltipRenderState {
                anchor: center,
                content,
            });
            let (successor, _) = draw(&mut tree, tooltip.as_ref());
            let has_text = |expected: &str| {
                leaves(&successor.root).any(|node| matches!(node, SceneNode::Text(text) if text.text == expected))
            };
            assert_eq!(manager.is_visible(), tip.is_some());
            if let Some(tip) = tip {
                assert!(has_text(tip));
            } else {
                for inactive in ["Help text", "Above target", "Right of target", "Auto placement"] {
                    assert!(!has_text(inactive));
                }
            }
        }
    }
}

fn overlay_menu_items() -> Vec<MenuItem> {
    vec![
        MenuItem::ActionRow {
            id: 1,
            label: "Open".into(),
            icon: None,
            shortcut: Some("Enter".into()),
            disabled: false,
            submenu: None,
        },
        MenuItem::ActionRow {
            id: 2,
            label: "Inspect".into(),
            icon: None,
            shortcut: Some("I".into()),
            disabled: false,
            submenu: None,
        },
        MenuItem::Separator,
        MenuItem::ActionRow {
            id: 4,
            label: "Locked action".into(),
            icon: None,
            shortcut: None,
            disabled: true,
            submenu: None,
        },
        MenuItem::ActionRow {
            id: 3,
            label: "Dismiss".into(),
            icon: None,
            shortcut: Some("Esc".into()),
            disabled: false,
            submenu: None,
        },
    ]
}

fn overlay_menu_label(id: u32) -> &'static str {
    match id {
        1 => "Open",
        2 => "Inspect",
        3 => "Dismiss",
        4 => "Locked action",
        _ => "Unknown",
    }
}

fn add_overlay_caption(
    cx: &mut ScreenFrameContext<'_>,
    parent: NodeId,
    z: i16,
    text: impl Into<String>,
) {
    cx.tree
        .add_child(parent, Box::new(SectionHeader::new(text.into())), None, z);
}

fn add_overlay_section(
    cx: &mut ScreenFrameContext<'_>,
    parent: NodeId,
    z: i16,
    title: impl Into<String>,
    subtitle: impl Into<String>,
) -> i16 {
    cx.tree
        .add_child(parent, Box::new(SectionHeader::new(title.into())), None, z);
    cx.tree.add_child(
        parent,
        Box::new(AuxiliaryText::new(subtitle.into())),
        None,
        z + 1,
    );
    cx.tree.add_child(
        parent,
        Box::new(Divider::horizontal().with_color(cx.app.theme.colors.borders.default)),
        None,
        z + 2,
    );
    z + 3
}

fn add_overlay_section_gap(cx: &mut ScreenFrameContext<'_>, parent: NodeId, z: i16) -> i16 {
    cx.tree
        .add_child(parent, Box::new(FixedSpacer::vertical(4.0)), None, z);
    z + 1
}

fn child_rect(cx: &mut ChildWindowPaintContext<'_>, rect: RectF, color: Color, z: ZIndex) {
    let scale = cx.scene.scale_factor();
    cx.scene
        .push_rect(rect.snap_to_physical_pixels(scale), color, z, None);
}

fn child_one_physical_px(scale: f32) -> f32 {
    1.0 / scale
}

fn child_horizontal_rule(
    cx: &mut ChildWindowPaintContext<'_>,
    x: f32,
    y: f32,
    width: f32,
    color: Color,
    z: ZIndex,
) {
    let scale = cx.scene.scale_factor();
    let x0 = snap_to_physical(x, scale);
    let x1 = snap_to_physical(x + width, scale);
    let y0 = snap_to_physical(y, scale);
    cx.scene.push_rect(
        RectF::new(x0, y0, (x1 - x0).max(0.0), child_one_physical_px(scale)),
        color,
        z,
        None,
    );
}

fn child_vertical_rule(
    cx: &mut ChildWindowPaintContext<'_>,
    x: f32,
    y: f32,
    height: f32,
    color: Color,
    z: ZIndex,
) {
    let scale = cx.scene.scale_factor();
    let x0 = snap_to_physical(x, scale);
    let y0 = snap_to_physical(y, scale);
    let y1 = snap_to_physical(y + height, scale);
    cx.scene.push_rect(
        RectF::new(x0, y0, child_one_physical_px(scale), (y1 - y0).max(0.0)),
        color,
        z,
        None,
    );
}

fn child_border(cx: &mut ChildWindowPaintContext<'_>, rect: RectF, color: Color, z: ZIndex) {
    let px = child_one_physical_px(cx.scene.scale_factor());
    cx.scene.push_border_rect(rect, color, px, z, None);
}

fn child_sculpted_rect(
    cx: &mut ChildWindowPaintContext<'_>,
    rect: RectF,
    fill: Color,
    raised: bool,
    z: ZIndex,
) {
    child_rect(cx, rect, fill, z);
    let (near_edge, far_edge) = if raised {
        (
            cx.theme.colors.borders.highlight,
            cx.theme.colors.borders.shadow,
        )
    } else {
        (
            cx.theme.colors.borders.shadow,
            cx.theme.colors.borders.highlight,
        )
    };
    let px = child_one_physical_px(cx.scene.scale_factor());
    child_horizontal_rule(cx, rect.x, rect.y, rect.width, near_edge, z + 1);
    child_vertical_rule(cx, rect.x, rect.y, rect.height, near_edge, z + 1);
    child_horizontal_rule(cx, rect.x, rect.bottom() - px, rect.width, far_edge, z + 1);
    child_vertical_rule(cx, rect.right() - px, rect.y, rect.height, far_edge, z + 1);
}

impl ShowcaseScreen {
    pub(super) fn build_overlays(&mut self, cx: &mut ScreenFrameContext<'_>, parent: NodeId) {
        let writer = self.binder.writer();
        let menu_state = self
            .overlay_context_menu
            .as_ref()
            .map(|(handle, _)| {
                if handle.borrow().is_open() {
                    "open"
                } else {
                    "closing"
                }
            })
            .unwrap_or("closed");
        let dialog_state = self
            .overlay_dialog_handle
            .as_ref()
            .map(|handle| if handle.is_open() { "open" } else { "closing" })
            .unwrap_or("closed");
        let menu_open = menu_state == "open";
        let dialog_open = dialog_state == "open";
        let toast_stats = cx.app.toast_manager.borrow().stats();
        let toast_live =
            toast_stats.active > 0 || toast_stats.pending > 0 || toast_stats.queue_len > 0;
        let toast_state = if toast_live { "active" } else { "idle" };
        self.overlay_toast_manager = Some(Rc::clone(&cx.app.toast_manager));

        let scroll = cx
            .tree
            .add_child(parent, Box::new(ScrollArea::new(NodeId(0))), None, 0);
        cx.tree.set_flex(scroll, FlexParams::flexible(1.0));

        let outer = cx.tree.add_child(
            scroll,
            Box::new(
                Flex::column()
                    .with_spacing(8.0)
                    .with_padding(EdgeInsets::new(12.0, 12.0, 28.0, 12.0)),
            ),
            None,
            0,
        );
        halcyon::replace_widget!(cx.tree, scroll, ScrollArea::new(outer));

        cx.tree.add_child(
            outer,
            Box::new(SectionHeader::new("Overlays and Windows")),
            None,
            0,
        );
        cx.tree.add_child(
            outer,
            Box::new(AuxiliaryText::new(
                "Transient UI surfaces, real modal routing, and verified multi-window runtime state.",
            )),
            None,
            1,
        );

        let mut z = add_overlay_section(
            cx,
            outer,
            2,
            "Surface controls",
            "Actions exercise ownership; live routing is summarized below.",
        );

        let command_row =
            cx.tree
                .add_child(outer, Box::new(Flex::row().with_spacing(14.0)), None, z);
        z += 1;

        let tooltip_block = cx.tree.add_child(
            command_row,
            Box::new(Flex::column().with_spacing(6.0)),
            None,
            0,
        );
        cx.tree.set_flex(tooltip_block, FlexParams::fixed(360.0));
        add_overlay_caption(cx, tooltip_block, 0, "Tooltips");
        let tooltip_row = cx.tree.add_child(
            tooltip_block,
            Box::new(Flex::row().with_spacing(8.0)),
            None,
            1,
        );
        for (i, &(label, tip, placement, tooltip_enabled)) in [
            ("North", "Above target", TooltipPlacement::Above, true),
            ("East", "Right of target", TooltipPlacement::Right, true),
            ("Cursor", "Auto placement", TooltipPlacement::Auto, true),
            ("Disabled", "Help text", TooltipPlacement::Auto, false),
        ]
        .iter()
        .enumerate()
        {
            cx.tree.add_child(
                tooltip_row,
                Box::new(TooltipTarget::new(label, tip, placement, tooltip_enabled)),
                None,
                i as i16,
            );
        }

        let overlay_buttons = cx.tree.add_child(
            command_row,
            Box::new(Flex::column().with_spacing(6.0)),
            None,
            1,
        );
        cx.tree.set_flex(overlay_buttons, FlexParams::flexible(1.0));
        add_overlay_caption(cx, overlay_buttons, 0, "Commands");

        let button_row_a = cx.tree.add_child(
            overlay_buttons,
            Box::new(Flex::row().with_spacing(8.0)),
            None,
            1,
        );
        let menu_button_host = cx
            .tree
            .add_child(button_row_a, Box::new(Label::new("")), None, 0);
        self.overlay_menu_button_id = Some(menu_button_host);
        let menu_button_label = cx.tree.add_child(
            menu_button_host,
            Box::new(Label::new("Open Context Menu")),
            None,
            0,
        );
        cx.tree.replace_widget_only(
            menu_button_host,
            Box::new(
                Button::new(menu_button_label)
                    .with_binding("overlay_open_context_menu", writer.clone()),
            ),
        );

        let modal_button_host = cx
            .tree
            .add_child(button_row_a, Box::new(Label::new("")), None, 1);
        let modal_button_label = cx.tree.add_child(
            modal_button_host,
            Box::new(Label::new("Open Modal")),
            None,
            0,
        );
        cx.tree.replace_widget_only(
            modal_button_host,
            Box::new(
                Button::new(modal_button_label).with_binding("overlay_open_dialog", writer.clone()),
            ),
        );

        let strict_button_host = cx
            .tree
            .add_child(button_row_a, Box::new(Label::new("")), None, 2);
        let strict_button_label = cx.tree.add_child(
            strict_button_host,
            Box::new(Label::new("Strict Modal")),
            None,
            0,
        );
        cx.tree.replace_widget_only(
            strict_button_host,
            Box::new(
                Button::new(strict_button_label)
                    .with_binding("overlay_open_strict_dialog", writer.clone()),
            ),
        );

        let button_row_b = cx.tree.add_child(
            overlay_buttons,
            Box::new(Flex::row().with_spacing(8.0)),
            None,
            2,
        );
        let toast_row = cx.tree.add_child(
            button_row_b,
            Box::new(Flex::row().with_spacing(8.0)),
            None,
            0,
        );
        let toast_button_host = cx
            .tree
            .add_child(toast_row, Box::new(Label::new("")), None, 0);
        let toast_button_label = cx.tree.add_child(
            toast_button_host,
            Box::new(Label::new("Push Toast")),
            None,
            0,
        );
        cx.tree.replace_widget_only(
            toast_button_host,
            Box::new(
                Button::new(toast_button_label).with_binding("overlay_push_toast", writer.clone()),
            ),
        );

        let action_toast_button_host =
            cx.tree
                .add_child(toast_row, Box::new(Label::new("")), None, 1);
        let action_toast_button_label = cx.tree.add_child(
            action_toast_button_host,
            Box::new(Label::new("Action Toast")),
            None,
            0,
        );
        cx.tree.replace_widget_only(
            action_toast_button_host,
            Box::new(
                Button::new(action_toast_button_label)
                    .with_binding("overlay_push_action_toast", writer.clone()),
            ),
        );

        let burst_toast_button_host =
            cx.tree
                .add_child(toast_row, Box::new(Label::new("")), None, 2);
        let burst_toast_button_label = cx.tree.add_child(
            burst_toast_button_host,
            Box::new(Label::new("Burst x5")),
            None,
            0,
        );
        cx.tree.replace_widget_only(
            burst_toast_button_host,
            Box::new(
                Button::new(burst_toast_button_label)
                    .with_binding("overlay_push_toast_burst", writer.clone()),
            ),
        );

        let dismiss_toast_button_host =
            cx.tree
                .add_child(toast_row, Box::new(Label::new("")), None, 3);
        let dismiss_toast_button_label = cx.tree.add_child(
            dismiss_toast_button_host,
            Box::new(Label::new("Dismiss")),
            None,
            0,
        );
        cx.tree.replace_widget_only(
            dismiss_toast_button_host,
            Box::new(
                Button::new(dismiss_toast_button_label)
                    .with_binding("overlay_dismiss_toasts", writer.clone()),
            ),
        );

        let button_row_c = cx.tree.add_child(
            overlay_buttons,
            Box::new(Flex::row().with_spacing(8.0)),
            None,
            3,
        );
        let child_window_button_host =
            cx.tree
                .add_child(button_row_c, Box::new(Label::new("")), None, 0);
        let child_window_button_label = cx.tree.add_child(
            child_window_button_host,
            Box::new(Label::new("Open Child Window")),
            None,
            0,
        );
        cx.tree.replace_widget_only(
            child_window_button_host,
            Box::new(
                Button::new(child_window_button_label)
                    .with_binding("overlay_open_child_window", writer.clone()),
            ),
        );

        z = add_overlay_section(
            cx,
            outer,
            z,
            "Live state",
            "Active surfaces, child-window state, and queue counters in one aligned readout.",
        );
        let child_state = self
            .overlay_child_window_id
            .map(|id| format!("open {:?}", id))
            .unwrap_or_else(|| "closed".to_string());
        let live_grid = cx.tree.add_child(
            outer,
            Box::new(
                StatusGrid::new(vec![
                    StatusGridCell::new("MENU", menu_state),
                    StatusGridCell::new("DIALOG", dialog_state),
                    StatusGridCell::new("TOAST", toast_state),
                    StatusGridCell::new("PARENT", "WindowId(0)"),
                    StatusGridCell::new("CHILD", child_state.as_str()),
                    StatusGridCell::new("VALUE", self.overlay_child_value.as_str()),
                ])
                .with_columns(3)
                .with_wide_rows(vec![StatusGridWideRow::new(
                    "LAST ACTION",
                    self.overlay_status.as_str(),
                )])
                .with_metrics(vec![
                    StatusGridMetric::new("PUSHED", toast_stats.total_pushed.to_string()),
                    StatusGridMetric::new("QUEUE", toast_stats.queue_len.to_string()),
                    StatusGridMetric::new("ACTIVE", toast_stats.active.to_string()),
                    StatusGridMetric::new("VISIBLE", toast_stats.visible.to_string()),
                    StatusGridMetric::new("DISMISSED", toast_stats.dismissed.to_string()),
                    StatusGridMetric::new("FLUSHED", toast_stats.flushed_dismissed.to_string()),
                ])
                .with_selection_policy(TextSelectionPolicy::Enabled),
            ),
            None,
            z,
        );
        self.overlay_status_grid_id = Some(live_grid);
        cx.tree.set_flex(live_grid, FlexParams::fixed(148.0));
        z += 1;
        z = add_overlay_section_gap(cx, outer, z);

        z = add_overlay_section(
            cx,
            outer,
            z,
            "Window demo",
            "In-process OS window, fixed-size child surface, SignalBus return path.",
        );
        let window_demo_table = cx.tree.add_child(
            outer,
            Box::new(
                ReadoutTable::new(
                    vec![
                        ReadoutColumn::new("PATH", 170.0),
                        ReadoutColumn::new("STATE", 150.0),
                        ReadoutColumn::new("PAYLOAD", 240.0),
                        ReadoutColumn::new("WINDOW POLICY", 210.0),
                    ],
                    vec![
                        vec![
                            ReadoutCell::new("Parent -> Child", false),
                            ReadoutCell::new(child_state.clone(), true),
                            ReadoutCell::new(self.overlay_child_value.clone(), true),
                            ReadoutCell::new("fixed OS size", true),
                        ],
                        vec![
                            ReadoutCell::new("Confirm", false),
                            ReadoutCell::new("DialogConfirm", true),
                            ReadoutCell::new("value returns to parent", true),
                            ReadoutCell::new("child closes", true),
                        ],
                        vec![
                            ReadoutCell::new("Cancel / Escape", false),
                            ReadoutCell::new("DialogCancel", true),
                            ReadoutCell::new("no mutation", true),
                            ReadoutCell::new("child closes", true),
                        ],
                    ],
                )
                .with_selection_policy(TextSelectionPolicy::Enabled),
            ),
            None,
            z,
        );
        cx.tree.set_flex(window_demo_table, FlexParams::fixed(92.0));
        z += 1;
        z = add_overlay_section_gap(cx, outer, z);

        z = add_overlay_section(
            cx,
            outer,
            z,
            "Routing matrix",
            "Each row is a surface contract: owner, input boundary, dismissal, and render path.",
        );
        let dialog_input = if dialog_open {
            "dialog capture"
        } else {
            "modal on open"
        };
        let dialog_dismissal = if dialog_open {
            if self.overlay_dialog_strict {
                "strict policy"
            } else {
                "Esc/outside noop-click"
            }
        } else {
            "configured policy"
        };
        let menu_input = if menu_open {
            "menu top hit"
        } else {
            "anchor pending"
        };
        let routing_table = cx.tree.add_child(
            outer,
            Box::new(
                ReadoutTable::new(
                    vec![
                        ReadoutColumn::new("SURFACE", 92.0),
                        ReadoutColumn::new("OWNER", 150.0),
                        ReadoutColumn::new("INPUT", 154.0),
                        ReadoutColumn::new("DISMISSAL", 190.0),
                        ReadoutColumn::new("RENDER", 150.0),
                    ],
                    vec![
                        vec![
                            ReadoutCell::new("Dialog", false),
                            ReadoutCell::new("ModalStack", true),
                            ReadoutCell::new(dialog_input, true),
                            ReadoutCell::new(dialog_dismissal, true),
                            ReadoutCell::new("modal overlay", true),
                        ],
                        vec![
                            ReadoutCell::new("Menu", false),
                            ReadoutCell::new("ContextMenuScreen", true),
                            ReadoutCell::new(menu_input, true),
                            ReadoutCell::new("outside/Esc closes", true),
                            ReadoutCell::new("overlay route", true),
                        ],
                        vec![
                            ReadoutCell::new("Tooltip", false),
                            ReadoutCell::new("TooltipManager", true),
                            ReadoutCell::new("hover target", true),
                            ReadoutCell::new("leave/timeout", true),
                            ReadoutCell::new("paint overlay", true),
                        ],
                        vec![
                            ReadoutCell::new("Toast", false),
                            ReadoutCell::new("ToastManager", true),
                            ReadoutCell::new("passive lane", true),
                            ReadoutCell::new("timeout/action", true),
                            ReadoutCell::new("toast lane", true),
                        ],
                    ],
                )
                .with_selection_policy(TextSelectionPolicy::Enabled),
            ),
            None,
            z,
        );
        cx.tree.set_flex(routing_table, FlexParams::fixed(114.0));
        z += 1;
        z = add_overlay_section_gap(cx, outer, z);

        z = add_overlay_section(
            cx,
            outer,
            z,
            "Subsystem ledger",
            "Runtime authorities and scene boundaries exposed as a compact ledger.",
        );
        let ledger_table = cx.tree.add_child(
            outer,
            Box::new(
                ReadoutTable::new(
                    vec![
                        ReadoutColumn::new("AREA", 144.0),
                        ReadoutColumn::new("AUTHORITY", 162.0),
                        ReadoutColumn::new("STATE", 168.0),
                        ReadoutColumn::new("NOTE", 270.0),
                    ],
                    vec![
                        vec![
                            ReadoutCell::new("Surface scenes", false),
                            ReadoutCell::new("SurfaceId", true),
                            ReadoutCell::new("one scene each", true),
                            ReadoutCell::new("projection pulled before render", true),
                        ],
                        vec![
                            ReadoutCell::new("Multi-window", false),
                            ReadoutCell::new("shared device", true),
                            ReadoutCell::new(
                                if self.overlay_child_window_id.is_some() {
                                    "child open"
                                } else {
                                    "ready"
                                },
                                true,
                            ),
                            ReadoutCell::new("App::run child window", true),
                        ],
                        vec![
                            ReadoutCell::new("Signals", false),
                            ReadoutCell::new("SignalStore/Bus", true),
                            ReadoutCell::new("before scene", true),
                            ReadoutCell::new("guarded against re-entry", true),
                        ],
                        vec![
                            ReadoutCell::new("Toast lane", false),
                            ReadoutCell::new("ToastManager", true),
                            ReadoutCell::new(if toast_live { "active" } else { "idle" }, true),
                            ReadoutCell::new("passive; never captures focus", true),
                        ],
                    ],
                )
                .with_selection_policy(TextSelectionPolicy::Enabled),
            ),
            None,
            z,
        );
        cx.tree.set_flex(ledger_table, FlexParams::fixed(114.0));
        z += 1;

        cx.tree.add_child(
            outer,
            Box::new(Divider::horizontal().with_color(cx.app.theme.colors.borders.default)),
            None,
            z + 1,
        );
        let policy_strip = cx.tree.add_child(
            outer,
            Box::new(
                KeyValueStrip::new(vec![
                    KeyValueItem::new("NORMAL MODAL", "outside dismiss consumes click"),
                    KeyValueItem::new("STRICT MODAL", "outside input blocked"),
                    KeyValueItem::new("KEYBOARD", "modal focus owns navigation"),
                    KeyValueItem::new("CONTEXT MENU", "anchor or pointer"),
                ])
                .with_columns(2)
                .with_selection_policy(TextSelectionPolicy::Enabled),
            ),
            None,
            z + 2,
        );
        cx.tree.set_flex(policy_strip, FlexParams::fixed(58.0));
    }

    pub(super) fn paint_overlay_child_window_scene(
        &self,
        window: &ChildWindowSceneState,
        cx: &mut ChildWindowPaintContext<'_>,
    ) -> bool {
        if self.overlay_child_window_id != Some(window.window_id) {
            return false;
        }

        let scale = cx.scene.scale_factor();
        let bounds = window.bounds.snap_to_physical_pixels(scale);
        let frame = bounds.shrink(8.0).snap_to_physical_pixels(scale);
        let header_h = 28.0;
        let route_h = 24.0;
        let label_h = 13.0;
        let field_h = 28.0;
        let payload_h = 24.0;
        let footer_h = 34.0;

        child_rect(cx, bounds, cx.theme.colors.surfaces.base0, 0);
        child_rect(cx, frame, cx.theme.colors.surfaces.base0, 1);
        child_border(cx, frame, cx.theme.colors.borders.strong, 4);

        let header_rect = RectF::new(frame.x, frame.y, frame.width, header_h);
        child_rect(cx, header_rect, cx.theme.colors.surfaces.level1, 2);
        let px = child_one_physical_px(scale);
        child_horizontal_rule(
            cx,
            frame.x,
            header_rect.bottom() - px,
            frame.width,
            cx.theme.colors.borders.strong,
            5,
        );
        let title_run = cx.text_engine.shape("SHOWCASE CHILD WINDOW", 12.0);
        cx.text_engine.draw(
            &title_run,
            header_rect.x + 8.0,
            header_rect.y + (header_rect.height - title_run.height) * 0.5,
            cx.theme.colors.typography.primary,
            CHILD_WINDOW_TEXT_LAYER,
            cx.scene,
        );

        let route_rect = RectF::new(
            frame.x + 8.0,
            header_rect.bottom() + 6.0,
            frame.width - 16.0,
            route_h,
        )
        .snap_to_physical_pixels(scale);
        child_rect(cx, route_rect, cx.theme.colors.surfaces.level1, 2);
        child_border(cx, route_rect, cx.theme.colors.borders.subtle, 4);
        let route_label = cx.text_engine.shape("ROUTE", 10.0);
        cx.text_engine.draw(
            &route_label,
            route_rect.x + 6.0,
            route_rect.y + (route_rect.height - route_label.height) * 0.5,
            cx.theme.colors.typography.muted,
            CHILD_WINDOW_TEXT_LAYER,
            cx.scene,
        );
        let route_text = format!(
            "Parent {:?}  ->  Child {:?}",
            window.parent_window_id, window.window_id
        );
        let route_run = cx.text_engine.shape(&route_text, 11.0);
        cx.text_engine.draw(
            &route_run,
            route_rect.x + 64.0,
            route_rect.y + (route_rect.height - route_run.height) * 0.5,
            cx.theme.colors.typography.secondary,
            CHILD_WINDOW_TEXT_LAYER,
            cx.scene,
        );

        let content_x = frame.x + 10.0;
        let content_w = frame.width - 20.0;
        let value_text = if window.edit_buffer.is_empty() {
            "<empty>"
        } else {
            &window.edit_buffer
        };

        let value_label_y = route_rect.bottom() + 8.0;
        let value_label = cx.text_engine.shape("CURRENT VALUE", 10.0);
        cx.text_engine.draw(
            &value_label,
            content_x,
            value_label_y,
            cx.theme.colors.typography.primary,
            CHILD_WINDOW_TEXT_LAYER,
            cx.scene,
        );
        let field_rect = RectF::new(content_x, value_label_y + label_h, content_w, field_h)
            .snap_to_physical_pixels(scale);
        child_sculpted_rect(cx, field_rect, cx.theme.colors.surfaces.level1, false, 2);
        let field_clip = field_rect.shrink(4.0);
        cx.scene.begin_clip(field_clip, None);
        let value_run = cx.text_engine.shape(value_text, 13.0);
        cx.text_engine.draw(
            &value_run,
            field_rect.x + 8.0,
            field_rect.y + (field_rect.height - value_run.height) * 0.5,
            cx.theme.colors.typography.primary,
            CHILD_WINDOW_TEXT_LAYER,
            cx.scene,
        );
        cx.scene.end_clip();

        let payload_label_y = field_rect.bottom() + 8.0;
        let payload_label = cx.text_engine.shape("SIGNAL PAYLOAD", 10.0);
        cx.text_engine.draw(
            &payload_label,
            content_x,
            payload_label_y,
            cx.theme.colors.typography.primary,
            CHILD_WINDOW_TEXT_LAYER,
            cx.scene,
        );
        let payload_rect = RectF::new(content_x, payload_label_y + label_h, content_w, payload_h)
            .snap_to_physical_pixels(scale);
        child_sculpted_rect(cx, payload_rect, cx.theme.colors.surfaces.level1, false, 2);
        let payload_text = format!("DialogConfirm(value = \"{}\")", value_text);
        let payload_clip = payload_rect.shrink(4.0);
        cx.scene.begin_clip(payload_clip, None);
        let payload_run = cx.text_engine.shape(&payload_text, 11.0);
        cx.text_engine.draw(
            &payload_run,
            payload_rect.x + 8.0,
            payload_rect.y + (payload_rect.height - payload_run.height) * 0.5,
            cx.theme.colors.typography.secondary,
            CHILD_WINDOW_TEXT_LAYER,
            cx.scene,
        );
        cx.scene.end_clip();

        let footer_rect = RectF::new(frame.x, frame.bottom() - footer_h, frame.width, footer_h)
            .snap_to_physical_pixels(scale);
        child_rect(cx, footer_rect, cx.theme.colors.surfaces.level1, 1);
        child_horizontal_rule(
            cx,
            frame.x,
            footer_rect.y,
            frame.width,
            cx.theme.colors.borders.strong,
            5,
        );
        let hint = "Edit locally; Confirm returns value.";
        let hint_run = cx.text_engine.shape(hint, 10.0);
        cx.text_engine.draw(
            &hint_run,
            footer_rect.x + 8.0,
            footer_rect.y + (footer_rect.height - hint_run.height) * 0.5,
            cx.theme.colors.typography.secondary,
            CHILD_WINDOW_TEXT_LAYER,
            cx.scene,
        );

        let button_h = 24.0;
        let button_gap = 6.0;
        let cancel_w = 70.0;
        let confirm_w = 98.0;
        let buttons_y = footer_rect.y + (footer_rect.height - button_h) * 0.5;
        let confirm_rect = RectF::new(
            footer_rect.right() - 8.0 - confirm_w,
            buttons_y,
            confirm_w,
            button_h,
        )
        .snap_to_physical_pixels(scale);
        let cancel_rect = RectF::new(
            confirm_rect.x - button_gap - cancel_w,
            buttons_y,
            cancel_w,
            button_h,
        )
        .snap_to_physical_pixels(scale);
        cx.action_regions.push(ChildWindowActionRegion {
            action: ChildWindowAction::Cancel,
            rect: cancel_rect,
        });
        cx.action_regions.push(ChildWindowActionRegion {
            action: ChildWindowAction::Confirm,
            rect: confirm_rect,
        });
        let cancel_hovered = window.hovered_action == Some(ChildWindowAction::Cancel);
        let cancel_armed = window.armed_action == Some(ChildWindowAction::Cancel);
        let confirm_hovered = window.hovered_action == Some(ChildWindowAction::Confirm);
        let confirm_armed = window.armed_action == Some(ChildWindowAction::Confirm);
        let cancel_fill = if cancel_armed {
            cx.theme.colors.controls.background_pressed
        } else if cancel_hovered {
            cx.theme.colors.controls.background_hover
        } else {
            cx.theme.colors.controls.background
        };
        let confirm_fill = if confirm_armed {
            cx.theme.colors.accents.hover
        } else if confirm_hovered {
            cx.theme.colors.accents.hover
        } else {
            cx.theme.colors.accents.primary
        };

        child_sculpted_rect(cx, cancel_rect, cancel_fill, !cancel_armed, 3);
        if cancel_hovered {
            child_border(cx, cancel_rect, cx.theme.colors.controls.border_hover, 6);
        }
        child_sculpted_rect(cx, confirm_rect, confirm_fill, !confirm_armed, 3);
        if confirm_hovered {
            child_border(cx, confirm_rect, cx.theme.colors.borders.focus, 6);
        }

        let cancel_run = cx.text_engine.shape("Cancel", 11.0);
        cx.text_engine.draw(
            &cancel_run,
            cancel_rect.x + (cancel_rect.width - cancel_run.width) * 0.5,
            cancel_rect.y + (cancel_rect.height - cancel_run.height) * 0.5,
            cx.theme.colors.controls.text,
            CHILD_WINDOW_TEXT_LAYER,
            cx.scene,
        );
        let confirm_run = cx.text_engine.shape("Confirm", 11.0);
        cx.text_engine.draw(
            &confirm_run,
            confirm_rect.x + (confirm_rect.width - confirm_run.width) * 0.5,
            confirm_rect.y + (confirm_rect.height - confirm_run.height) * 0.5,
            cx.theme.colors.typography.inverse,
            CHILD_WINDOW_TEXT_LAYER,
            cx.scene,
        );

        true
    }

    pub(super) fn handle_overlays_input(
        &mut self,
        event: &FusedInputEvent,
        cx: &mut ScreenContext<'_>,
    ) -> bool {
        match event {
            FusedInputEvent::PointerDown {
                button: PointerButton::Secondary,
                pos,
                ..
            } => {
                self.open_overlay_context_menu(cx, Anchor::Point(*pos));
                true
            }
            _ => false,
        }
    }

    pub(super) fn flush_overlay_actions(&mut self, cx: &mut ScreenContext<'_>) {
        let selection_id = self
            .overlay_context_menu
            .as_ref()
            .and_then(|(_, sink)| sink.take());
        let menu_dismissed = self
            .overlay_context_menu
            .as_ref()
            .is_some_and(|(handle, _)| !handle.borrow().is_open());

        if let Some(id) = selection_id {
            self.overlay_status = format!("Selected: {}", overlay_menu_label(id));
            self.overlay_context_menu = None;
        } else if menu_dismissed {
            self.overlay_status = "Dismissed".to_string();
            self.overlay_context_menu = None;
        }

        if let Some(handle) = &self.overlay_dialog_handle {
            if !handle.is_open() {
                if let Some(key) = handle.pressed_key() {
                    self.overlay_status = format!("Dialog result: {}", key);
                } else {
                    self.overlay_status = "Dialog dismissed".to_string();
                }
                self.overlay_dialog_handle = None;
                self.overlay_dialog_strict = false;
                self.needs_rebuild = true;
            }
        }

        let mut open_menu = false;
        let mut open_dialog = false;
        let mut open_strict_dialog = false;
        let mut push_toast = false;
        let mut push_action_toast = false;
        let mut push_toast_burst = false;
        let mut dismiss_toasts = false;
        let mut open_child_window = false;
        self.binder.bind(cx, |b| {
            if b.bool("overlay_open_context_menu").take().is_some() {
                open_menu = true;
            }
            if b.bool("overlay_open_dialog").take().is_some() {
                open_dialog = true;
            }
            if b.bool("overlay_open_strict_dialog").take().is_some() {
                open_strict_dialog = true;
            }
            if b.bool("overlay_push_toast").take().is_some() {
                push_toast = true;
            }
            if b.bool("overlay_push_action_toast").take().is_some() {
                push_action_toast = true;
            }
            if b.bool("overlay_push_toast_burst").take().is_some() {
                push_toast_burst = true;
            }
            if b.bool("overlay_dismiss_toasts").take().is_some() {
                dismiss_toasts = true;
            }
            if b.bool("overlay_open_child_window").take().is_some() {
                open_child_window = true;
            }
        });

        if open_menu {
            if let Some(anchor) = self.overlay_menu_button_anchor(cx) {
                self.open_overlay_context_menu(cx, anchor);
            }
        }

        if open_dialog {
            self.open_overlay_dialog(
                cx,
                DialogConfig {
                    title: "Overlay Modal".to_string(),
                    body: "Dialog surface opened through nav_open_modal. Outside click and Escape both dismiss."
                        .to_string(),
                    buttons: vec![
                        DialogButton {
                            label: "Cancel".to_string(),
                            key: "cancel".to_string(),
                            is_default: false,
                            is_cancel: true,
                        },
                        DialogButton {
                            label: "Apply".to_string(),
                            key: "apply".to_string(),
                            is_default: true,
                            is_cancel: false,
                        },
                    ],
                    dismiss_on_escape: true,
                    dismiss_on_outside_click: true,
                },
                "Dialog opened",
                false,
            );
        }

        if open_strict_dialog {
            self.open_overlay_dialog(
                cx,
                DialogConfig {
                    title: "Strict Modal".to_string(),
                    body: "This modal captures the full background. Outside click is not a dismissal path."
                        .to_string(),
                    buttons: vec![
                        DialogButton {
                            label: "Cancel".to_string(),
                            key: "cancel".to_string(),
                            is_default: false,
                            is_cancel: true,
                        },
                        DialogButton {
                            label: "Run".to_string(),
                            key: "run".to_string(),
                            is_default: true,
                            is_cancel: false,
                        },
                    ],
                    dismiss_on_escape: false,
                    dismiss_on_outside_click: false,
                },
                "Strict dialog opened",
                true,
            );
        }

        if push_toast {
            cx.push_toast("Overlay toast from AppRoot queue.", 4000, None);
            self.overlay_status = "Toast pushed".to_string();
            self.needs_rebuild = true;
        }

        if push_action_toast {
            cx.push_toast(
                "Overlay action toast.",
                6000,
                Some(ToastAction {
                    label: "Review".to_string(),
                }),
            );
            self.overlay_status = "Action toast pushed".to_string();
            self.needs_rebuild = true;
        }

        if push_toast_burst {
            for i in 1..=5 {
                cx.push_toast(format!("Overlay burst toast {i}"), 5000, None);
            }
            self.overlay_status = "Toast burst pushed".to_string();
            self.needs_rebuild = true;
        }

        if dismiss_toasts {
            cx.app.toast_manager.borrow_mut().dismiss_all();
            self.overlay_status = "Visible toasts dismissed".to_string();
            self.needs_rebuild = true;
        }

        if open_child_window {
            if let Some(id) = self.overlay_child_window_id {
                self.overlay_status = format!("Child {:?} already open", id);
            } else {
                let id = cx.request_child_window(
                    "Showcase Child Window",
                    520,
                    236,
                    self.overlay_child_value.clone(),
                );
                self.overlay_child_window_id = Some(id);
                self.overlay_status = format!(
                    "Child {:?} requested; type letters, Enter confirms, Escape cancels",
                    id
                );
            }
            self.needs_rebuild = true;
        }
    }

    fn open_overlay_dialog(
        &mut self,
        cx: &mut ScreenContext<'_>,
        config: DialogConfig,
        status: &str,
        strict: bool,
    ) {
        let handle = DialogHandle::new();
        self.overlay_dialog_handle = Some(handle.clone());
        self.overlay_dialog_strict = strict;
        cx.nav_open_modal(Box::new(Dialog::new(config, handle)));
        self.overlay_status = status.to_string();
        self.needs_rebuild = true;
    }

    fn overlay_menu_button_anchor(&self, cx: &mut ScreenContext<'_>) -> Option<Anchor> {
        let node = self.overlay_menu_button_id?;
        let rect = cx.tree.as_ref()?.get_absolute_paint_rect(node)?;
        if rect.width <= 0.0 || rect.height <= 0.0 {
            return None;
        }
        Some(Anchor::Rect(rect))
    }

    fn open_overlay_context_menu(&mut self, cx: &mut ScreenContext<'_>, anchor: Anchor) {
        if let Some((handle, _)) = &self.overlay_context_menu {
            handle.borrow_mut().dismiss();
        }
        let (handle, sink) = context_menu_handle();
        handle.borrow_mut().show(anchor, overlay_menu_items());
        cx.nav_open_modal(Box::new(ContextMenuScreen::new(handle.clone())));
        self.overlay_context_menu = Some((handle, sink));
        self.overlay_status = "Context menu open".to_string();
    }
}
