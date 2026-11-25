// SPEC-HAL-218: DropdownMenu Widget
// Action selector with integrated trigger + overlay popup.
// Trigger is button-like (label + caret). Popup delegates to MenuSurface (SPEC-HAL-223).
// No value display, no selection memory. Emits action to external sink.

use std::cell::Cell;

use crate::core::event::{Event, EventPhase, KeyCode, PointerButton, PointerEventKind};
use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{RectF, Size};
use crate::core::widget::{EventContext, LayoutContext, PaintContext, Widget, WidgetDowncast};
use crate::core::widget_tree::NodeId;
use crate::model::dropdown_menu::DropdownMenuHandle;
use crate::model::menu_item::MenuItem;
use crate::utils::id::{TreeWidgetId, WidgetInternalId};
use crate::widgets::menu_surface::MenuSurfaceOverlay;
use crate::widgets::trigger_chrome::{
    TriggerChromeAdornments, TriggerChromeFlags, paint_trigger_chrome,
};

const TRIGGER_HEIGHT: f32 = 30.0;
const POPUP_MIN_WIDTH: f32 = 120.0;

pub struct DropdownMenu {
    id: WidgetInternalId,
    controller: DropdownMenuHandle,
    label: String,
    actions: Vec<MenuItem>,
    disabled: bool,
    focused: bool,
    hovered: bool,
    overlay: MenuSurfaceOverlay,
    /// Cached trigger rect from paint (absolute coords) for open_menu().
    cached_trigger_rect: Cell<RectF>,
}

impl DropdownMenu {
    pub fn new(controller: DropdownMenuHandle, label: &str) -> Self {
        Self {
            id: WidgetInternalId::new(),
            controller,
            label: label.to_string(),
            actions: Vec::new(),
            disabled: false,
            focused: false,
            hovered: false,
            overlay: MenuSurfaceOverlay::new(),
            cached_trigger_rect: Cell::new(RectF::new(0.0, 0.0, 0.0, 0.0)),
        }
    }

    pub fn set_actions(&mut self, actions: Vec<MenuItem>) {
        self.actions = actions;
    }

    pub fn set_disabled(&mut self, disabled: bool) {
        self.disabled = disabled;
    }

    fn open_menu(&mut self) {
        let trigger_rect = self.cached_trigger_rect.get();
        let items: Vec<MenuItem> = self.actions.clone();
        self.controller.borrow_mut().show(trigger_rect, items);
    }
}

impl WidgetDowncast for DropdownMenu {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for DropdownMenu {
    fn on_mount(&mut self) {
        self.hovered = false;
    }

    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        let width = if constraints.max_width.is_finite() {
            constraints.max_width
        } else {
            POPUP_MIN_WIDTH
        };
        LayoutResult::fit(Size::new(width, TRIGGER_HEIGHT))
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        // Cache trigger rect for open_menu() and handle_event offset
        self.cached_trigger_rect.set(rect);

        paint_trigger_chrome(
            cx,
            rect,
            &self.label,
            &TriggerChromeFlags {
                enabled: !self.disabled,
                hovered: !self.disabled && self.hovered,
                pressed: false,
                // SPEC-HAL-012/328: current framework owner and cue eligibility.
                focused: !self.disabled && cx.has_focus_cue(),
                open: self.controller.borrow().is_open(),
            },
            &TriggerChromeAdornments { caret: true },
        );
    }

    fn paint_overlay(&self, cx: &mut PaintContext, rect: RectF, tree_widget_id: TreeWidgetId) {
        let ctrl = self.controller.borrow();
        if !ctrl.is_open() {
            return;
        }

        // Popup anchored below trigger. Min width includes trigger width.
        let min_width = rect.width.max(POPUP_MIN_WIDTH);
        self.overlay.paint(
            cx,
            ctrl.surface(),
            (rect.x, rect.y + rect.height),
            min_width,
            tree_widget_id,
            Some(rect),
            rect,
        );
    }

    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        if phase != EventPhase::Target {
            return false;
        }
        if self.disabled {
            return false;
        }

        let is_open = self.controller.borrow().is_open();

        if is_open {
            // Popup open: delegate all events to overlay (with keyboard navigation enabled).
            let mut ctrl = self.controller.borrow_mut();
            if let Some(consumed) = self.overlay.handle_event(ctrl.surface_mut(), event, true) {
                if consumed {
                    cx.invalidate_layout();
                }
                return consumed;
            }
            // Event not handled by overlay — fall through.
        }

        // Popup closed (or event not handled by overlay): handle trigger interactions.
        match event {
            Event::Pointer(pe) if pe.kind == PointerEventKind::Down && !is_open => {
                if pe.button == Some(PointerButton::Primary) {
                    self.open_menu();
                    return true;
                }
            }
            Event::Key(ke) if ke.is_down && !is_open => match ke.key_code {
                KeyCode::Enter | KeyCode::Space | KeyCode::Down => {
                    self.open_menu();
                    return true;
                }
                _ => {}
            },
            _ => {}
        }
        false
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn focusable(&self) -> bool {
        !self.disabled
    }

    fn on_hover_enter(&mut self, cx: &mut EventContext) {
        if !self.disabled && !self.hovered {
            self.hovered = true;
            cx.invalidate_paint();
        }
    }

    fn on_hover_leave(&mut self, cx: &mut EventContext) {
        if self.hovered {
            self.hovered = false;
            cx.invalidate_paint();
        }
    }

    fn on_focus(&mut self, _cx: &mut EventContext) {
        self.focused = true;
    }

    fn on_blur(&mut self, _cx: &mut EventContext) {
        self.focused = false;
        if self.controller.borrow().is_open() {
            self.controller.borrow_mut().dismiss();
        }
    }

    fn accepts_keyboard_activation(&self) -> bool {
        !self.disabled
    }

    fn on_keyboard_activate(&mut self, cx: &mut EventContext) {
        if self.disabled {
            return;
        }
        if !self.controller.borrow().is_open() {
            self.open_menu();
            cx.invalidate_paint();
        }
    }

    fn hit_testable(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dropdown_menu_focus_cue_follows_framework_owner_and_policy() {
        use crate::core::app::{RetainedSceneTestFrame, RetainedSceneTestHarness};
        use crate::core::event::{
            FusedInputEvent, InteractionDomains, PointerButtonsState, PointerDevice, PointerId,
            UiInteractionPolicy, WindowId,
        };
        use crate::core::event_router::EventRouter;
        use crate::core::icons::builtin_registry;
        use crate::core::paint::LAYER_CONTENT;
        use crate::core::scene::SceneNode;
        use crate::core::text_engine::TextEngine;
        use crate::core::theme::Theme;
        use crate::core::widget_tree::WidgetTree;
        use crate::model::dropdown_menu::dropdown_menu_handle;
        use crate::platform::font_manager_text_engine::FontManagerTextEngine;
        use crate::runtime::binding_registry::BindingRegistry;
        use crate::widgets::{flex::Flex, label::Label};
        use std::rc::Rc;
        use std::sync::Arc;
        use web_time::Instant;

        fn menu(tree: &WidgetTree, node: NodeId) -> &DropdownMenu {
            tree.node(node)
                .unwrap()
                .widget
                .as_any()
                .downcast_ref()
                .unwrap()
        }
        fn composition(frame: &RetainedSceneTestFrame) -> Arc<[SceneNode]> {
            let [SceneNode::RetainedFragment(root)] = frame.scene_root.as_slice() else {
                panic!()
            };
            root.children.clone()
        }
        let (controller, sink) = dropdown_menu_handle();
        let mut trigger = DropdownMenu::new(controller.clone(), "Actions");
        trigger.set_actions(vec![MenuItem::ActionRow {
            id: 7,
            label: "Run action".into(),
            icon: None,
            shortcut: None,
            disabled: false,
            submenu: None,
        }]);
        let (disabled_controller, disabled_sink) = dropdown_menu_handle();
        let mut disabled_trigger = DropdownMenu::new(disabled_controller.clone(), "Actions");
        disabled_trigger.set_actions(trigger.actions.clone());
        disabled_trigger.set_disabled(true);
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(Flex::column().with_spacing(12.0)), None, 0);
        let node = tree.add_child(root, Box::new(trigger), None, 0);
        let peer = tree.add_child(root, Box::new(Label::new("Stable peer")), None, 1);
        let disabled_node = tree.add_child(root, Box::new(disabled_trigger), None, 2);
        let [root_id, owner, peer_id, disabled_owner] =
            [root, node, peer, disabled_node].map(|id| tree.node(id).unwrap().id);
        let geometry = |tree: &WidgetTree| {
            [root, node, peer, disabled_node].map(|id| {
                let entry = tree.node(id).unwrap();
                (
                    entry.id,
                    entry.widget.id(),
                    tree.get_absolute_paint_rect(id).unwrap(),
                    entry.layout_epoch,
                )
            })
        };
        let mut theme = Theme::default();
        theme.focus_ring.focused_width = 3.0;
        let icons = builtin_registry();
        let mut text = FontManagerTextEngine::new();
        let mut router = EventRouter::new();
        let mut harness = RetainedSceneTestHarness::new();
        let mut pointer_domains =
            InteractionDomains::with_ui_interaction_policy(UiInteractionPolicy::PointerDriven);
        macro_rules! frame {
            ($domains:expr) => {
                harness.frame_with_appearance(
                    &mut tree,
                    root,
                    Constraints::tight(Size::new(300.0, 140.0)),
                    &theme,
                    &mut text,
                    &icons,
                    $domains,
                    WindowId::PRIMARY,
                )
            };
        }
        let initial = frame!(&pointer_domains);
        let bounds = geometry(&tree);
        let rect = bounds[1].2;
        let disabled_rect = bounds[3].2;
        let hit = |tree: &WidgetTree| {
            let result = tree
                .hit_test(rect.x + 2.0, rect.y + rect.height * 0.5)
                .unwrap();
            (result.widget_id, result.local_pos, result.absolute_bounds)
        };
        let original_hit = hit(&tree);
        let ordinary = |nodes: &[SceneNode]| {
            let fills = nodes
                .iter()
                .filter_map(|node| match node {
                    SceneNode::Rect(rect) => Some((rect.rect, rect.color, rect.z_index)),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let borders = nodes
                .iter()
                .filter_map(|node| match node {
                    SceneNode::BorderRect(border) if border.z_index != 5 => Some((
                        border.rect,
                        border.fill_color,
                        border.border_color,
                        border.border_width,
                        border.radius,
                        border.z_index,
                    )),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let labels = nodes
                .iter()
                .filter_map(|node| match node {
                    SceneNode::Text(text) => Some((
                        text.text.clone(),
                        text.position,
                        text.font_size,
                        text.color,
                        text.z_index,
                    )),
                    _ => None,
                })
                .collect::<Vec<_>>();
            (fills, borders, labels)
        };
        let mut previous = harness.local_fragment(owner).unwrap();
        let resting = ordinary(&previous);
        assert_eq!(
            resting.0,
            vec![(rect, theme.colors.list.row_background.into(), LAYER_CONTENT)]
        );
        assert_eq!(resting.1.len(), 1);
        assert_eq!(resting.1[0].0, rect);
        assert_eq!(resting.1[0].2, theme.colors.borders.default.into());
        assert_eq!(
            (resting.1[0].3, resting.1[0].4, resting.1[0].5),
            (1.0, 0.0, LAYER_CONTENT)
        );
        assert_eq!(resting.1[0].1.a, 0.0);
        assert_eq!(
            resting
                .2
                .iter()
                .map(|text| text.0.to_string())
                .collect::<Vec<_>>(),
            vec!["Actions".to_string(), "\u{25BE}".to_string()]
        );
        let peer_fragment = harness.local_fragment(peer_id).unwrap();
        let root_fragment = harness.local_fragment(root_id).unwrap();
        let disabled_fragment = harness.local_fragment(disabled_owner).unwrap();
        let disabled_snapshot = ordinary(&disabled_fragment);
        let disabled_value = menu(&tree, disabled_node);
        assert!(disabled_value.disabled);
        assert!(!disabled_value.focusable() && !disabled_value.accepts_keyboard_activation());
        assert!(!disabled_value.focused);
        assert!(
            disabled_fragment
                .iter()
                .all(|node| !matches!(node, SceneNode::BorderRect(border) if border.z_index == 5))
        );
        let translate = |source: RectF| {
            RectF::new(
                source.x + disabled_rect.x - rect.x,
                source.y + disabled_rect.y - rect.y,
                source.width,
                source.height,
            )
        };
        assert_eq!(
            disabled_snapshot.0,
            resting
                .0
                .iter()
                .map(|(rect, color, z)| (translate(*rect), *color, *z))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            disabled_snapshot.1,
            resting
                .1
                .iter()
                .map(|(rect, fill, border, width, radius, z)| (
                    translate(*rect),
                    *fill,
                    *border,
                    *width,
                    *radius,
                    *z
                ))
                .collect::<Vec<_>>()
        );
        assert_eq!(disabled_snapshot.2.len(), 2);
        for (disabled, enabled) in disabled_snapshot.2.iter().zip(&resting.2) {
            assert_eq!(disabled.0, enabled.0);
            assert_eq!(
                disabled.1,
                crate::core::math::Point::new(
                    enabled.1.x + disabled_rect.x - rect.x,
                    enabled.1.y + disabled_rect.y - rect.y,
                )
            );
            assert_eq!((disabled.2, disabled.4), (enabled.2, enabled.4));
            let enabled_metrics = text.font_metrics(enabled.2);
            let disabled_metrics = text.font_metrics(disabled.2);
            assert_eq!(
                (disabled_metrics.ascent, disabled_metrics.descent),
                (enabled_metrics.ascent, enabled_metrics.descent)
            );
            let enabled_run = text.shape(&enabled.0, enabled.2);
            let disabled_run = text.shape(&disabled.0, disabled.2);
            assert_eq!(
                (disabled_run.width, disabled_run.height),
                (enabled_run.width, enabled_run.height)
            );
            assert_eq!(enabled.3, theme.colors.list.text.into());
            assert_eq!(
                disabled.3,
                theme.colors.typography.muted.into(),
                "disabled trigger content {} must use the existing muted theme role",
                disabled.0
            );
        }
        let mut composed = composition(&initial);
        let check_projection = |nodes: &[SceneNode], focused: bool, phase: &str| {
            let rings = nodes
                .iter()
                .filter_map(|node| match node {
                    SceneNode::BorderRect(border) if border.z_index == 5 => Some(border),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(
                rings.len(),
                usize::from(focused),
                "{phase}: trigger cue must follow current framework owner and construction-time policy"
            );
            if focused {
                let ring = rings[0];
                assert_eq!(ring.rect, rect);
                assert_eq!(ring.border_color, theme.colors.borders.focus.into());
                assert_eq!(ring.border_width, theme.focus_ring.focused_width);
                assert_eq!(ring.radius, 0.0);
                assert_eq!(
                    (
                        ring.fill_color.r,
                        ring.fill_color.g,
                        ring.fill_color.b,
                        ring.fill_color.a
                    ),
                    (0.0, 0.0, 0.0, 0.0)
                );
                assert_eq!(ring.z_index, 5);
            }
            assert_eq!(ordinary(nodes), resting);
        };
        let check_stable = |tree: &WidgetTree, harness: &RetainedSceneTestHarness| {
            assert_eq!(geometry(tree), bounds);
            assert_eq!(hit(tree), original_hit);
            assert!(Arc::ptr_eq(
                &peer_fragment,
                &harness.local_fragment(peer_id).unwrap()
            ));
            assert!(Arc::ptr_eq(
                &root_fragment,
                &harness.local_fragment(root_id).unwrap()
            ));
            assert!(Arc::ptr_eq(
                &disabled_fragment,
                &harness.local_fragment(disabled_owner).unwrap()
            ));
            assert_eq!(
                ordinary(&harness.local_fragment(disabled_owner).unwrap()),
                disabled_snapshot
            );
            let disabled = menu(tree, disabled_node);
            assert!(disabled.disabled && !disabled.focused);
            assert!(!disabled.focusable() && !disabled.accepts_keyboard_activation());
            assert!(!disabled.needs_update() && !disabled.needs_animation());
            assert_eq!(disabled.cached_trigger_rect.get(), disabled_rect);
            assert_eq!(disabled.label, "Actions");
            assert!(Rc::ptr_eq(&disabled.controller, &disabled_controller));
            let [
                MenuItem::ActionRow {
                    id,
                    label,
                    icon,
                    shortcut,
                    disabled,
                    submenu,
                },
            ] = disabled.actions.as_slice()
            else {
                panic!()
            };
            assert_eq!((*id, label.as_str(), *disabled), (7, "Run action", false));
            assert!(icon.is_none() && shortcut.is_none() && submenu.is_none());
            let disabled_ctrl = disabled_controller.borrow();
            assert!(!disabled_ctrl.is_open() && disabled_ctrl.items().is_empty());
            assert_eq!(disabled_ctrl.anchor_rect(), None);
            assert_eq!(disabled_sink.get(), None);
            let value = menu(tree, node);
            assert_eq!(value.label, "Actions");
            assert!(!value.disabled);
            assert_eq!(value.cached_trigger_rect.get(), rect);
            assert!(Rc::ptr_eq(&value.controller, &controller));
            assert!(!value.capture_eligible());
            assert!(value.paints_own_focus_indicator());
            assert!(!value.needs_update() && !value.needs_animation());
            let [
                MenuItem::ActionRow {
                    id,
                    label,
                    icon,
                    shortcut,
                    disabled,
                    submenu,
                },
            ] = value.actions.as_slice()
            else {
                panic!()
            };
            assert_eq!((*id, label.as_str(), *disabled), (7, "Run action", false));
            assert!(icon.is_none() && shortcut.is_none() && submenu.is_none());
            let ctrl = controller.borrow();
            assert!(!ctrl.is_open() && ctrl.items().is_empty());
            assert_eq!(ctrl.anchor_rect(), None);
            assert_eq!(sink.get(), None);
            assert!(!tree.node(node).unwrap().dirty_layout);
        };
        let changed = |result: &RetainedSceneTestFrame,
                       harness: &RetainedSceneTestHarness,
                       previous: &Arc<[SceneNode]>,
                       composed: &Arc<[SceneNode]>| {
            assert!(result.scene_built && !result.needs_next_frame);
            assert!(!Arc::ptr_eq(
                previous,
                &harness.local_fragment(owner).unwrap()
            ));
            let next = composition(result);
            assert!(!Arc::ptr_eq(composed, &next));
            assert!(Arc::ptr_eq(
                &disabled_fragment,
                &harness.local_fragment(disabled_owner).unwrap()
            ));
            next
        };
        macro_rules! idle {
            ($domains:expr) => {{
                let local = harness.local_fragment(owner).unwrap();
                let idle = frame!($domains);
                assert!(!idle.scene_built && !idle.visual_changed && !idle.needs_next_frame);
                assert!(Arc::ptr_eq(&local, &harness.local_fragment(owner).unwrap()));
                assert!(!router.continuous_frames_requested());
                check_stable(&tree, &harness);
            }};
        }
        assert!(!menu(&tree, node).focused);
        check_projection(&previous, false, "unfocused PointerDriven");
        router.request_focus_for_widget(&mut tree, owner, pointer_domains.primary_state_mut());
        assert_eq!(
            pointer_domains.primary_state().focus_manager.focused(),
            Some(owner)
        );
        assert!(menu(&tree, node).focused);
        let pointer = frame!(&pointer_domains);
        let fragment = harness.local_fragment(owner).unwrap();
        check_projection(&fragment, false, "first focused PointerDriven frame");
        assert!(menu(&tree, node).focused);
        composed = changed(&pointer, &harness, &previous, &composed);
        previous = fragment;
        assert!(router.continuous_frames_requested());
        router.reset_continuous_frames_request();
        idle!(&pointer_domains);

        let mut navigation_domains = InteractionDomains::new();
        router.request_focus_for_widget(&mut tree, owner, navigation_domains.primary_state_mut());
        assert_eq!(
            navigation_domains.primary_state().focus_manager.focused(),
            Some(owner)
        );
        let navigation = frame!(&navigation_domains);
        let fragment = harness.local_fragment(owner).unwrap();
        check_projection(&fragment, true, "first focused NavigationAndCapture frame");
        assert!(menu(&tree, node).focused);
        composed = changed(&navigation, &harness, &previous, &composed);
        previous = fragment;
        assert!(router.continuous_frames_requested());
        router.reset_continuous_frames_request();
        idle!(&navigation_domains);

        navigation_domains
            .primary_state_mut()
            .focus_manager
            .set_focus(None);
        assert!(menu(&tree, node).focused);
        assert!(!tree.node(node).unwrap().dirty_paint);
        assert!(!tree.node(root).unwrap().dirty_composition);
        let lost = frame!(&navigation_domains);
        let fragment = harness.local_fragment(owner).unwrap();
        check_projection(&fragment, false, "owner None without widget callback");
        assert!(menu(&tree, node).focused);
        composed = changed(&lost, &harness, &previous, &composed);
        previous = fragment;
        idle!(&navigation_domains);

        navigation_domains
            .primary_state_mut()
            .focus_manager
            .set_focus(Some(owner));
        assert!(menu(&tree, node).focused);
        assert!(!tree.node(node).unwrap().dirty_paint);
        assert!(!tree.node(root).unwrap().dirty_composition);
        let restored = frame!(&navigation_domains);
        let fragment = harness.local_fragment(owner).unwrap();
        check_projection(&fragment, true, "owner restored without widget callback");
        assert!(menu(&tree, node).focused);
        changed(&restored, &harness, &previous, &composed);
        idle!(&navigation_domains);
        assert_eq!(
            navigation_domains.primary_state().focus_manager.focused(),
            Some(owner)
        );
        assert!(
            navigation_domains
                .primary_state()
                .pointer_capture_manager
                .current_capture(PointerId::MOUSE)
                .is_none()
        );
        assert!(
            pointer_domains
                .primary_state()
                .pointer_capture_manager
                .current_capture(PointerId::MOUSE)
                .is_none()
        );
        assert_eq!(sink.get(), None);

        let focus_borders = |nodes: &[SceneNode]| {
            nodes
                .iter()
                .filter_map(|node| match node {
                    SceneNode::BorderRect(border) if border.z_index == 5 => Some((
                        border.rect,
                        border.fill_color,
                        border.border_color,
                        border.border_width,
                        border.radius,
                        border.z_index,
                    )),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        let focused_fragment = harness.local_fragment(owner).unwrap();
        let focused_borders = focus_borders(&focused_fragment);
        let hover_composed = composition(&restored);
        let mut bindings = BindingRegistry::new();
        let time = Instant::now();
        let pointer_move = |rect: RectF, x: f32| FusedInputEvent::PointerMove {
            window_id: WindowId::PRIMARY,
            time,
            pos: crate::core::math::Point::new(rect.x + x, rect.y + rect.height * 0.5),
            delta: (0.0, 0.0),
            device: PointerDevice::Mouse,
            buttons: PointerButtonsState {
                primary: false,
                secondary: false,
                auxiliary: false,
            },
        };
        assert_ne!(
            theme.colors.list.row_hover,
            theme.colors.list.row_background
        );
        router.route_input_event(
            &mut tree,
            pointer_move(rect, 2.0),
            Some(root),
            &mut text,
            None,
            navigation_domains.primary_state_mut(),
            &mut bindings,
        );
        assert_eq!(
            navigation_domains.primary_state().hovered_widget,
            Some(owner)
        );
        let hovered = frame!(&navigation_domains);
        let hover_fragment = harness.local_fragment(owner).unwrap();
        assert_eq!(
            ordinary(&hover_fragment).0,
            vec![(rect, theme.colors.list.row_hover.into(), LAYER_CONTENT)],
            "routed enabled hover must immediately use the existing row_hover background"
        );
        let mut hover_ordinary = resting.clone();
        hover_ordinary.0[0].1 = theme.colors.list.row_hover.into();
        assert_eq!(ordinary(&hover_fragment), hover_ordinary);
        assert_eq!(focus_borders(&hover_fragment), focused_borders);
        let hover_composed = changed(&hovered, &harness, &focused_fragment, &hover_composed);
        check_stable(&tree, &harness);
        assert!(!router.continuous_frames_requested());
        idle!(&navigation_domains);

        router.route_input_event(
            &mut tree,
            pointer_move(rect, 4.0),
            Some(root),
            &mut text,
            None,
            navigation_domains.primary_state_mut(),
            &mut bindings,
        );
        assert_eq!(
            navigation_domains.primary_state().hovered_widget,
            Some(owner)
        );
        assert!(!tree.node(node).unwrap().dirty_paint);
        idle!(&navigation_domains);
        assert!(Arc::ptr_eq(
            &hover_fragment,
            &harness.local_fragment(owner).unwrap()
        ));

        router.clear_hover(&mut tree, navigation_domains.primary_state_mut());
        assert_eq!(navigation_domains.primary_state().hovered_widget, None);
        let cleared = frame!(&navigation_domains);
        let cleared_fragment = harness.local_fragment(owner).unwrap();
        check_projection(
            &cleared_fragment,
            true,
            "clear_hover retains framework focus",
        );
        assert_eq!(focus_borders(&cleared_fragment), focused_borders);
        changed(&cleared, &harness, &hover_fragment, &hover_composed);
        check_stable(&tree, &harness);
        idle!(&navigation_domains);

        router.route_input_event(
            &mut tree,
            pointer_move(disabled_rect, 2.0),
            Some(root),
            &mut text,
            None,
            navigation_domains.primary_state_mut(),
            &mut bindings,
        );
        assert_eq!(
            navigation_domains.primary_state().hovered_widget,
            Some(disabled_owner)
        );
        let disabled_enter = frame!(&navigation_domains);
        assert!(disabled_enter.scene_built && !disabled_enter.needs_next_frame);
        let disabled_enter_fragment = harness.local_fragment(disabled_owner).unwrap();
        assert_eq!(ordinary(&disabled_enter_fragment), disabled_snapshot);
        assert!(focus_borders(&disabled_enter_fragment).is_empty());
        assert!(Arc::ptr_eq(
            &cleared_fragment,
            &harness.local_fragment(owner).unwrap()
        ));
        assert!(Arc::ptr_eq(
            &peer_fragment,
            &harness.local_fragment(peer_id).unwrap()
        ));
        assert!(Arc::ptr_eq(
            &root_fragment,
            &harness.local_fragment(root_id).unwrap()
        ));
        assert_eq!(geometry(&tree), bounds);
        assert_eq!(hit(&tree), original_hit);
        let disabled_hit = tree
            .hit_test(
                disabled_rect.x + 2.0,
                disabled_rect.y + disabled_rect.height * 0.5,
            )
            .unwrap();
        assert_eq!(disabled_hit.widget_id, disabled_owner);
        assert_eq!(
            disabled_hit.absolute_bounds,
            (
                disabled_rect.x,
                disabled_rect.y,
                disabled_rect.width,
                disabled_rect.height
            )
        );
        for (node, expected_disabled, controller, sink) in [
            (node, false, &controller, &sink),
            (disabled_node, true, &disabled_controller, &disabled_sink),
        ] {
            let value = menu(&tree, node);
            assert_eq!(value.disabled, expected_disabled);
            assert_eq!(value.label, "Actions");
            assert_eq!(value.actions.len(), 1);
            assert!(!value.needs_update() && !value.needs_animation());
            assert!(!tree.node(node).unwrap().dirty_layout);
            assert!(Rc::ptr_eq(&value.controller, controller));
            let ctrl = controller.borrow();
            assert!(!ctrl.is_open() && ctrl.items().is_empty());
            assert_eq!(ctrl.anchor_rect(), None);
            assert_eq!(sink.get(), None);
        }
        assert_eq!(
            navigation_domains.primary_state().focus_manager.focused(),
            Some(owner)
        );
        assert!(menu(&tree, node).focused && !menu(&tree, disabled_node).focused);
        assert!(
            navigation_domains
                .primary_state()
                .pointer_capture_manager
                .current_capture(PointerId::MOUSE)
                .is_none()
        );
        assert!(!router.continuous_frames_requested());
        let disabled_idle = frame!(&navigation_domains);
        assert!(
            !disabled_idle.scene_built
                && !disabled_idle.visual_changed
                && !disabled_idle.needs_next_frame
        );
        assert!(Arc::ptr_eq(
            &disabled_enter_fragment,
            &harness.local_fragment(disabled_owner).unwrap()
        ));
        assert!(Arc::ptr_eq(
            &cleared_fragment,
            &harness.local_fragment(owner).unwrap()
        ));
        assert!(Arc::ptr_eq(
            &peer_fragment,
            &harness.local_fragment(peer_id).unwrap()
        ));
        assert!(Arc::ptr_eq(
            &root_fragment,
            &harness.local_fragment(root_id).unwrap()
        ));
        assert_eq!(geometry(&tree), bounds);
        assert_eq!(hit(&tree), original_hit);
        assert!(!router.continuous_frames_requested());
    }
}
