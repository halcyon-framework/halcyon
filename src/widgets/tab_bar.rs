// SPEC-HAL-219: TabBar Control
// Pure navigation selector. Emits explicit selection intents.
// No content ownership, no layout authority, no routing.
//
// INV-A1: No content ownership.
// INV-A2: No layout authority.
// INV-B1: Single active tab.
// INV-B2: Explicit selection emission.
// INV-B3: No selection memory.
// INV-C1: Explicit activation only (pointer + keyboard).
// INV-C2: Keyboard parity.
// INV-C3: Re-selecting active tab is no-op.
// INV-D1: Tabs are visual labels.
// INV-D2: Geometry is self-contained.
// INV-D3: Overflow is explicit (clip).
// INV-E1: Disabled tab is inert.
// INV-E2: Disabled TabBar is absolute.
// INV-F1: Selection emission only.
// INV-F2: No form binding.
// INV-G1: Deterministic selection.
// INV-G2: No per-frame allocation.
// INV-G3: Invariant supremacy.

use crate::core::event::{Event, EventPhase, KeyCode, PointerButton, PointerEventKind};
use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{RectF, Size};
use crate::core::paint::{LAYER_CONTENT, LAYER_FOCUS};
use crate::core::widget::{EventContext, LayoutContext, PaintContext, Widget, WidgetDowncast};
use crate::core::widget_tree::NodeId;
use crate::model::tab_bar::TabBarSink;
use crate::utils::id::WidgetInternalId;

/// Opaque tab identity for external coordination.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TabId(pub u32);

pub struct TabBar {
    id: WidgetInternalId,
    /// External selection sink. Written on activation, read by caller.
    /// Widget retains zero knowledge of emitted values.
    sink: TabBarSink,
    /// Tab definitions: (id, label, enabled). Owned snapshot, replace-only.
    tabs: Vec<(TabId, String, bool)>,
    /// Externally provided active tab. TabBar does NOT own this.
    active_tab: Option<TabId>,
    /// Keyboard focus index within the tab list (roving focus).
    focus_index: usize,
    /// Global disabled state. When true, all input is rejected.
    disabled: bool,
    /// Cached tab rects from layout (relative to widget origin).
    /// Recomputed only on layout, not per-frame.
    tab_rects: Vec<RectF>,
    /// Horizontal scroll offset (px). Tabs are drawn shifted left by this.
    /// Overflow policy is scroll (supersedes the prior clip policy).
    scroll_x: f32,
    /// Total laid-out strip width (sum of tab widths), cached in layout.
    content_width: f32,
    /// Visible width of the strip (the widget's allotted width), cached in layout.
    viewport_width: f32,
    /// Index of the tab currently under the pointer, for hover highlight.
    /// `None` when the pointer is outside the strip or over a gap. Written on
    /// `PointerEventKind::Move`; cleared in `on_hover_leave`. Visual only.
    hovered_index: Option<usize>,
}

impl TabBar {
    pub fn new(sink: TabBarSink) -> Self {
        Self {
            id: WidgetInternalId::new(),
            sink,
            tabs: Vec::new(),
            active_tab: None,
            focus_index: 0,
            disabled: false,
            tab_rects: Vec::new(),
            scroll_x: 0.0,
            content_width: 0.0,
            viewport_width: 0.0,
            hovered_index: None,
        }
    }

    /// Replace tab definitions. Owned snapshot, replace-only.
    pub fn set_tabs(&mut self, tabs: &[(TabId, &str, bool)]) {
        self.tabs.clear();
        for &(id, label, enabled) in tabs {
            self.tabs.push((id, label.to_string(), enabled));
        }
        if !self.tabs.is_empty() && self.focus_index >= self.tabs.len() {
            self.focus_index = self.tabs.len() - 1;
        }
    }

    /// Set the externally-owned active tab.
    pub fn set_active_tab(&mut self, tab_id: Option<TabId>) {
        self.active_tab = tab_id;
    }

    /// Current horizontal scroll offset (px). A consumer that recreates the
    /// TabBar on rebuild can read this from the live bar and restore it via
    /// `set_scroll_x` so the strip does not snap back to the start.
    pub fn scroll_x(&self) -> f32 {
        self.scroll_x
    }

    /// Restore a previously captured horizontal scroll offset. The next
    /// `layout` clamps it to the current scrollable range.
    pub fn set_scroll_x(&mut self, scroll_x: f32) {
        self.scroll_x = scroll_x;
    }

    /// Set global disabled state.
    pub fn set_disabled(&mut self, disabled: bool) {
        self.disabled = disabled;
    }

    /// Attempt to activate the tab at the given index.
    /// Emits to external sink. Zero retention after write.
    /// No-op if: disabled, tab disabled, or already active (INV-C3).
    fn activate_tab(&mut self, index: usize) -> bool {
        if self.disabled {
            return false;
        }
        if let Some((id, _, enabled)) = self.tabs.get(index) {
            if !enabled {
                return false; // INV-E1: disabled tab is inert
            }
            if self.active_tab == Some(*id) {
                return false; // INV-C3: re-selecting active tab is no-op
            }
            self.sink.set(Some(*id));
            return true;
        }
        false
    }

    /// Move focus_index by delta, skipping disabled tabs.
    /// Clamps at boundaries (does not wrap).
    fn move_focus(&mut self, delta: isize) {
        if self.tabs.is_empty() {
            return;
        }
        let mut candidate = self.focus_index;
        for _ in 0..self.tabs.len() {
            let Some(next) = candidate.checked_add_signed(delta) else {
                break;
            };
            let Some((_, _, enabled)) = self.tabs.get(next) else {
                break;
            };
            if *enabled {
                self.focus_index = next;
                break;
            }
            candidate = next;
        }
        self.ensure_index_visible(self.focus_index);
    }

    /// Scroll the strip so the tab at `index` is fully visible (no-op if it
    /// already is). Clamps to the scrollable range. Uses geometry cached by
    /// the most recent layout.
    fn ensure_index_visible(&mut self, index: usize) {
        if let Some(r) = self.tab_rects.get(index).copied() {
            let vw = self.viewport_width;
            if r.x < self.scroll_x {
                self.scroll_x = r.x;
            } else if r.x + r.width > self.scroll_x + vw {
                self.scroll_x = r.x + r.width - vw;
            }
            let max_scroll = (self.content_width - vw).max(0.0);
            self.scroll_x = self.scroll_x.clamp(0.0, max_scroll);
        }
    }
}

impl WidgetDowncast for TabBar {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for TabBar {
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        let font_size = cx.theme.typography.font_size;
        let padding_x = cx.theme.geometry.button.padding_x;
        let height = cx.theme.geometry.button.height;

        self.tab_rects.clear();
        let mut cursor_x = 0.0f32;

        for (_, label, _) in &self.tabs {
            let run = cx.text_engine.shape(label, font_size);
            let tab_width = run.width + padding_x * 2.0;
            self.tab_rects
                .push(RectF::new(cursor_x, 0.0, tab_width, height));
            cursor_x += tab_width;
        }

        // INV-D2: Geometry is self-contained.
        // Overflow policy is scroll: the strip occupies at most the allotted
        // width and scrolls its content within it.
        let width = if constraints.max_width.is_finite() {
            cursor_x.min(constraints.max_width)
        } else {
            cursor_x
        };

        // Cache for scroll clamping (paint is &self and cannot mutate scroll_x).
        self.content_width = cursor_x;
        self.viewport_width = width;
        let max_scroll = (self.content_width - self.viewport_width).max(0.0);
        self.scroll_x = self.scroll_x.clamp(0.0, max_scroll);

        LayoutResult::fit(Size::new(width, height))
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        if self.tabs.is_empty() {
            return;
        }

        let font_size = cx.theme.typography.font_size;
        let controls = &cx.theme.colors.controls;

        // Clip to widget bounds; tabs are translated left by scroll_x (scroll
        // overflow policy). Content outside the viewport is clipped.
        cx.scene.begin_clip(rect, None);

        for (i, ((id, label, enabled), tab_rect)) in
            self.tabs.iter().zip(self.tab_rects.iter()).enumerate()
        {
            let abs_rect = RectF::new(
                rect.x + tab_rect.x - self.scroll_x,
                rect.y + tab_rect.y,
                tab_rect.width,
                tab_rect.height,
            );

            // Background
            let is_active = self.active_tab == Some(*id);
            let is_hovered = self.hovered_index == Some(i) && *enabled;
            let bg = if is_active {
                controls.background_pressed
            } else if is_hovered {
                controls.background_hover
            } else {
                controls.background
            };
            cx.scene.push_rect(abs_rect, bg, LAYER_CONTENT, None);

            // Active indicator: 2px bar at bottom
            if is_active {
                let indicator_height = 2.0;
                let indicator_rect = RectF::new(
                    abs_rect.x,
                    abs_rect.y + abs_rect.height - indicator_height,
                    abs_rect.width,
                    indicator_height,
                );
                cx.scene.push_rect(
                    indicator_rect,
                    cx.theme.colors.borders.focus,
                    LAYER_CONTENT + 1,
                    None,
                );
            }

            // Text label
            let text_color = if !*enabled {
                cx.theme.colors.typography.muted
            } else {
                controls.text
            };
            let run = cx.text_engine.shape(label, font_size);
            let text_x = abs_rect.x + (abs_rect.width - run.width) * 0.5;
            let text_y = abs_rect.y + (abs_rect.height - run.height) * 0.5;
            cx.scene.push_text_with_backgrounds(
                &run,
                text_x,
                text_y,
                text_color,
                vec![],
                LAYER_CONTENT + 1,
                None,
            );

            // Current keyboard ownership is distinct from selected content.
            let stroke = cx.theme.focus_ring.focused_width;
            if !self.disabled
                && *enabled
                && i == self.focus_index
                && cx.has_focus_cue()
                && stroke > 0.0
                && stroke.is_finite()
                && abs_rect.x.is_finite()
                && abs_rect.y.is_finite()
                && abs_rect.width > 0.0
                && abs_rect.width.is_finite()
                && abs_rect.height > 0.0
                && abs_rect.height.is_finite()
            {
                cx.scene.push_border_rect(
                    abs_rect,
                    cx.theme.colors.borders.focus,
                    stroke,
                    LAYER_FOCUS,
                    None,
                );
            }
        }

        cx.scene.end_clip();
    }

    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        if phase != EventPhase::Target {
            return false;
        }
        // INV-E2: Disabled TabBar is absolute.
        if self.disabled {
            return false;
        }

        match event {
            Event::Pointer(pe) => {
                match pe.kind {
                    // Wheel over the (hovered) TabBar scrolls the strip horizontally
                    // and is absorbed regardless: wants_scroll() makes the TabBar the
                    // sole scroll recipient under the cursor, so the event never
                    // reaches the content below. Vertical wheel maps to horizontal
                    // scroll; horizontal input is honored when present. Sign matches
                    // ScrollArea (offset += -delta).
                    PointerEventKind::Scroll { delta_x, delta_y } => {
                        let max_scroll = (self.content_width - self.viewport_width).max(0.0);
                        if max_scroll > 0.0 {
                            let d = if delta_x != 0.0 { delta_x } else { delta_y };
                            self.scroll_x = (self.scroll_x - d).clamp(0.0, max_scroll);
                        }
                        true
                    }
                    PointerEventKind::Down if pe.button == Some(PointerButton::Primary) => {
                        // Hit-test in content space: pointer-local x plus scroll offset.
                        let (lx, ly) = pe.local_pos;
                        let content_x = lx + self.scroll_x;
                        for (i, tab_rect) in self.tab_rects.iter().enumerate() {
                            if content_x >= tab_rect.x
                                && content_x < tab_rect.x + tab_rect.width
                                && ly >= tab_rect.y
                                && ly < tab_rect.y + tab_rect.height
                            {
                                self.focus_index = i;
                                self.activate_tab(i);
                                return true;
                            }
                        }
                        false
                    }
                    // Track which tab the pointer is over for hover highlight.
                    // Hit-test in content space (pointer-local x plus scroll
                    // offset), mirroring the Down arm. TabBar has no hover
                    // animation, so invalidate this owner's paint when the hovered tab
                    // changes. Not consumed: hover is visual only, so the
                    // framework's hover lifecycle still applies.
                    PointerEventKind::Move => {
                        let (lx, ly) = pe.local_pos;
                        let content_x = lx + self.scroll_x;
                        let new_hover = self.tab_rects.iter().position(|tab_rect| {
                            content_x >= tab_rect.x
                                && content_x < tab_rect.x + tab_rect.width
                                && ly >= tab_rect.y
                                && ly < tab_rect.y + tab_rect.height
                        });
                        if new_hover != self.hovered_index {
                            self.hovered_index = new_hover;
                            cx.invalidate_paint();
                        }
                        false
                    }
                    _ => false,
                }
            }
            Event::Key(ke) if ke.is_down => match ke.key_code {
                KeyCode::Left => {
                    self.move_focus(-1);
                    true
                }
                KeyCode::Right => {
                    self.move_focus(1);
                    true
                }
                _ => false,
            },
            _ => false,
        }
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn focusable(&self) -> bool {
        !self.disabled
    }

    fn on_focus(&mut self, _cx: &mut EventContext) {
        // Initialize focus to active tab if possible
        if let Some(active) = self.active_tab {
            if let Some(idx) = self.tabs.iter().position(|(id, _, _)| *id == active) {
                self.focus_index = idx;
            }
        }
    }

    /// Pointer left the strip: drop the hover highlight. Per-tab hover is
    /// tracked positionally in `handle_event` (`PointerEventKind::Move`); this
    /// clears it when the pointer is no longer over any tab and invalidates
    /// this owner's paint so the cleared state is drawn.
    fn on_hover_leave(&mut self, cx: &mut EventContext) {
        if self.hovered_index.is_some() {
            self.hovered_index = None;
            cx.invalidate_paint();
        }
    }

    fn accepts_keyboard_activation(&self) -> bool {
        !self.disabled
    }

    fn on_keyboard_activate(&mut self, cx: &mut EventContext) {
        if self.disabled {
            return;
        }
        if self.activate_tab(self.focus_index) {
            cx.request_continuous_frames();
        }
    }

    fn hit_testable(&self) -> bool {
        true
    }

    /// Receive wheel events when hovered so the strip scrolls and the event is
    /// absorbed (never leaks to the content below). See handle_event Scroll arm.
    fn wants_scroll(&self) -> bool {
        true
    }

    fn debug_type_name(&self) -> &'static str {
        "TabBar"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::app::RetainedSceneTestHarness;
    use crate::core::event::{
        FusedInputEvent, InteractionDomains, PointerButtonsState, PointerDevice, PointerId,
        WindowId,
    };
    use crate::core::event_router::EventRouter;
    use crate::core::math::Point;
    use crate::core::scene::{RectNode, SceneNode};
    use crate::core::theme::Theme;
    use crate::core::widget_tree::WidgetTree;
    use crate::model::tab_bar::tab_bar_sink;
    use crate::platform::font_manager_text_engine::FontManagerTextEngine;
    use crate::runtime::binding_registry::BindingRegistry;
    use crate::widgets::flex::Flex;
    use crate::widgets::label::Label;
    use std::sync::Arc;
    use std::time::Instant;

    #[test]
    fn tab_bar_keyboard_navigation_skips_disabled_boundaries() {
        use crate::core::event::{Modifiers, PhysicalKeyCode};
        use crate::core::text_engine::TextEngine;

        fn bar(tree: &WidgetTree, node: NodeId) -> &TabBar {
            tree.node(node)
                .unwrap()
                .widget
                .as_any()
                .downcast_ref()
                .unwrap()
        }
        fn clipped(nodes: &[SceneNode]) -> (RectF, &[SceneNode]) {
            let [SceneNode::Clip(clip)] = nodes else {
                panic!("tab navigation must keep the existing viewport clip")
            };
            (clip.rect, &clip.children)
        }

        let tabs = [
            (TabId(1), "Disabled start", false),
            (TabId(2), "First", true),
            (TabId(3), "Disabled middle", false),
            (TabId(4), "Second", true),
            (TabId(5), "Disabled end", false),
        ];
        let theme = Theme::dark();
        let mut text = FontManagerTextEngine::new();
        let width = tabs[..2]
            .iter()
            .map(|(_, label, _)| {
                text.shape(label, theme.typography.font_size).width
                    + 2.0 * theme.geometry.button.padding_x
            })
            .sum::<f32>();
        let constraints = Constraints::tight(Size::new(width, 120.0));
        let sink = tab_bar_sink();
        let mut control = TabBar::new(sink.clone());
        control.set_tabs(&tabs);
        control.set_active_tab(Some(TabId(2)));
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(Flex::column().with_spacing(12.0)), None, 0);
        let node = tree.add_child(root, Box::new(control), None, 0);
        let peer = tree.add_child(root, Box::new(Label::new("Unchanged peer")), None, 1);
        let owners = [root, node, peer].map(|id| tree.node(id).unwrap().id);
        let geometry = |tree: &WidgetTree| {
            [root, node, peer].map(|id| {
                let entry = tree.node(id).unwrap();
                (
                    entry.id,
                    entry.widget.id(),
                    tree.get_absolute_paint_rect(id).unwrap(),
                    entry.layout_epoch,
                )
            })
        };
        let mut domains = InteractionDomains::new();
        let mut router = EventRouter::new();
        let mut bindings = BindingRegistry::new();
        let icons = crate::core::icons::builtin_registry();
        let mut harness = RetainedSceneTestHarness::new();
        let frame = |tree: &mut WidgetTree,
                     harness: &mut RetainedSceneTestHarness,
                     text: &mut FontManagerTextEngine,
                     domains: &InteractionDomains| {
            harness.frame_with_appearance(
                tree,
                root,
                constraints,
                &theme,
                text,
                &icons,
                domains,
                WindowId::PRIMARY,
            )
        };
        frame(&mut tree, &mut harness, &mut text, &domains);
        let geometry_before = geometry(&tree);
        let origin = tree.get_absolute_paint_rect(node).unwrap();
        let rects = bar(&tree, node).tab_rects.clone();
        let definitions = bar(&tree, node).tabs.clone();
        let content_width = bar(&tree, node).content_width;
        assert!(rects[1].x + rects[1].width <= origin.width);
        assert!(rects[3].x + rects[3].width > origin.width);
        assert!(rects[3].width <= origin.width);
        assert!(content_width > origin.width);
        let hit_point = Point::new(origin.x + 1.0, origin.y + origin.height * 0.5);
        let hit = |tree: &WidgetTree| {
            let hit = tree.hit_test(hit_point.x, hit_point.y).unwrap();
            (hit.widget_id, hit.local_pos, hit.absolute_bounds)
        };
        let hit_before = hit(&tree);
        let peer_fragment = harness.local_fragment(owners[2]).unwrap();
        let root_fragment = harness.local_fragment(owners[0]).unwrap();
        let check_target = |tree: &WidgetTree, fragment: &[SceneNode], expected: TabId| {
            let control = bar(tree, node);
            let (clip, contents) = clipped(fragment);
            assert_eq!(clip, origin);
            let borders = contents
                .iter()
                .filter_map(|node| match node {
                    SceneNode::BorderRect(border) if border.z_index == LAYER_FOCUS => Some(border),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(
                borders.len(),
                1,
                "an enabled navigation target must keep its06 cue"
            );
            let current = &control.tabs[control.focus_index];
            assert_eq!(current.0, expected);
            assert!(current.2);
            let target = rects[control.focus_index];
            let target = RectF::new(
                origin.x + target.x - control.scroll_x,
                origin.y + target.y,
                target.width,
                target.height,
            );
            assert_eq!(borders[0].rect, target);
            assert_eq!(borders[0].border_color, theme.colors.borders.focus.into());
            assert_eq!(borders[0].border_width, theme.focus_ring.focused_width);
            assert!(target.x >= clip.x && target.x + target.width <= clip.x + clip.width);
            assert!(target.y >= clip.y && target.y + target.height <= clip.y + clip.height);
            let selected = contents
                .iter()
                .filter_map(|node| match node {
                    SceneNode::Rect(stripe) if stripe.z_index == LAYER_CONTENT + 1 => Some(stripe),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(selected.len(), 1);
            assert_eq!(selected[0].rect.x, origin.x + rects[1].x - control.scroll_x);
            assert_eq!(selected[0].rect.width, rects[1].width);
            assert_eq!(selected[0].color, theme.colors.borders.focus.into());
            assert!(selected[0].z_index < borders[0].z_index);
            assert_eq!(control.active_tab, Some(TabId(2)));
            assert_eq!(control.tabs, definitions);
            assert_eq!(control.tab_rects, rects);
            assert_eq!(control.content_width, content_width);
            assert_eq!(control.viewport_width, origin.width);
        };

        router.request_focus_for_widget(&mut tree, owners[1], domains.primary_state_mut());
        let focused = frame(&mut tree, &mut harness, &mut text, &domains);
        assert!(focused.scene_built && !focused.needs_next_frame);
        assert_eq!(bar(&tree, node).focus_index, 1);
        check_target(&tree, &harness.local_fragment(owners[1]).unwrap(), TabId(2));
        let settled = frame(&mut tree, &mut harness, &mut text, &domains);
        assert!(!settled.scene_built && !settled.needs_next_frame);
        assert!(router.continuous_frames_requested());
        router.reset_continuous_frames_request();
        let mut previous_scroll = bar(&tree, node).scroll_x;

        for (key_code, physical_key, expected, boundary) in [
            (KeyCode::Left, PhysicalKeyCode::ArrowLeft, TabId(2), true),
            (KeyCode::Right, PhysicalKeyCode::ArrowRight, TabId(4), false),
            (KeyCode::Right, PhysicalKeyCode::ArrowRight, TabId(4), true),
            (KeyCode::Left, PhysicalKeyCode::ArrowLeft, TabId(2), false),
        ] {
            router.route_input_event(
                &mut tree,
                FusedInputEvent::KeyDown {
                    window_id: WindowId::PRIMARY,
                    time: Instant::now(),
                    key_code,
                    physical_key,
                    reservation: None,
                    modifiers: Modifiers::default(),
                    repeat: false,
                },
                Some(root),
                &mut text,
                None,
                domains.primary_state_mut(),
                &mut bindings,
            );
            assert!(tree.node(node).unwrap().dirty_paint);
            assert!(tree.node(root).unwrap().dirty_composition);
            assert!(!tree.node(node).unwrap().dirty_layout);
            let current = frame(&mut tree, &mut harness, &mut text, &domains);
            let fragment = harness.local_fragment(owners[1]).unwrap();
            let control = bar(&tree, node);
            eprintln!("key={key_code:?} index={} id={:?} enabled={} scroll={} focus_borders={}",
                control.focus_index, control.tabs[control.focus_index].0,
                control.tabs[control.focus_index].2, control.scroll_x,
                clipped(&fragment).1.iter().filter(|node| matches!(node, SceneNode::BorderRect(border) if border.z_index == LAYER_FOCUS)).count());
            check_target(&tree, &fragment, expected);
            assert!(current.scene_built && !current.needs_next_frame);
            if boundary {
                assert_eq!(control.scroll_x, previous_scroll);
            }
            if expected == TabId(4) {
                assert!(control.scroll_x > 0.0);
            }
            previous_scroll = control.scroll_x;
            assert_eq!(sink.take(), None);
            assert_eq!(
                domains.primary_state().focus_manager.focused(),
                Some(owners[1])
            );
            assert!(
                domains
                    .primary_state()
                    .pointer_capture_manager
                    .current_capture(PointerId::MOUSE)
                    .is_none()
            );
            assert_eq!(geometry(&tree), geometry_before);
            assert_eq!(hit(&tree), hit_before);
            assert!(Arc::ptr_eq(
                &peer_fragment,
                &harness.local_fragment(owners[2]).unwrap()
            ));
            assert!(Arc::ptr_eq(
                &root_fragment,
                &harness.local_fragment(owners[0]).unwrap()
            ));
            // Consumed boundary arrows may repaint under the unchanged router policy.
            let idle = frame(&mut tree, &mut harness, &mut text, &domains);
            assert!(!idle.scene_built && !idle.visual_changed && !idle.needs_next_frame);
            assert!(Arc::ptr_eq(
                &fragment,
                &harness.local_fragment(owners[1]).unwrap()
            ));
            assert!(!router.continuous_frames_requested());
        }
    }

    #[test]
    fn tab_bar_focus_cue_tracks_framework_owner_and_internal_target() {
        use crate::core::event::UiInteractionPolicy;

        fn bar(tree: &WidgetTree, node: NodeId) -> &TabBar {
            tree.node(node)
                .unwrap()
                .widget
                .as_any()
                .downcast_ref()
                .unwrap()
        }
        fn clipped(nodes: &[SceneNode]) -> (RectF, &[SceneNode]) {
            let [SceneNode::Clip(clip)] = nodes else {
                panic!("the complete tab projection must stay inside its existing clip")
            };
            (clip.rect, &clip.children)
        }
        fn composition(frame: &crate::core::app::RetainedSceneTestFrame) -> Arc<[SceneNode]> {
            let [SceneNode::RetainedFragment(root)] = frame.scene_root.as_slice() else {
                panic!()
            };
            root.children.clone()
        }

        let sink = tab_bar_sink();
        let mut control = TabBar::new(sink.clone());
        control.set_tabs(&[
            (TabId(1), "Selected content", true),
            (TabId(2), "Other target", true),
        ]);
        control.set_active_tab(Some(TabId(1)));
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(Flex::column().with_spacing(12.0)), None, 0);
        let node = tree.add_child(root, Box::new(control), None, 0);
        let peer = tree.add_child(root, Box::new(Label::new("Unchanged sibling")), None, 1);
        let owners = [root, node, peer].map(|id| tree.node(id).unwrap().id);
        let geometry = |tree: &WidgetTree| {
            [root, node, peer].map(|id| {
                let entry = tree.node(id).unwrap();
                (
                    entry.id,
                    entry.widget.id(),
                    tree.get_absolute_paint_rect(id).unwrap(),
                    entry.layout_epoch,
                )
            })
        };
        let stable_state = |tree: &WidgetTree| {
            let value = bar(tree, node);
            (
                value.tabs.clone(),
                value.active_tab,
                value.disabled,
                value.tab_rects.clone(),
                value.scroll_x,
                value.content_width,
                value.viewport_width,
                value.hovered_index,
            )
        };
        let paint_state = |tree: &WidgetTree| (stable_state(tree), bar(tree, node).focus_index);
        let theme = Theme::dark();
        let mut text = FontManagerTextEngine::new();
        let icons = crate::core::icons::builtin_registry();
        let mut domains = InteractionDomains::new();
        let mut router = EventRouter::new();
        let mut bindings = BindingRegistry::new();
        let mut harness = RetainedSceneTestHarness::new();
        let frame = |tree: &mut WidgetTree,
                     harness: &mut RetainedSceneTestHarness,
                     text: &mut FontManagerTextEngine,
                     domains: &InteractionDomains| {
            harness.frame_with_appearance(
                tree,
                root,
                Constraints::tight(Size::new(500.0, 120.0)),
                &theme,
                text,
                &icons,
                domains,
                WindowId::PRIMARY,
            )
        };
        let rest = |fragment: &[SceneNode]| {
            let (_, contents) = clipped(fragment);
            let rects = contents
                .iter()
                .filter_map(|node| match node {
                    SceneNode::Rect(rect) => Some((rect.rect, rect.color, rect.z_index)),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let labels = contents
                .iter()
                .filter_map(|node| match node {
                    SceneNode::Text(label) => Some((
                        label.text.clone(),
                        label.position,
                        label.font_size,
                        label.color,
                        label.z_index,
                    )),
                    _ => None,
                })
                .collect::<Vec<_>>();
            (rects, labels)
        };
        let initial = frame(&mut tree, &mut harness, &mut text, &domains);
        assert!(initial.scene_built && !initial.needs_next_frame);
        let baseline = harness.local_fragment(owners[1]).unwrap();
        let ordinary = rest(&baseline);
        let origin = tree.get_absolute_paint_rect(node).unwrap();
        let targets = bar(&tree, node)
            .tab_rects
            .iter()
            .map(|rect| {
                RectF::new(
                    origin.x + rect.x - bar(&tree, node).scroll_x,
                    origin.y + rect.y,
                    rect.width,
                    rect.height,
                )
            })
            .collect::<Vec<_>>();
        let geometry_before = geometry(&tree);
        let state_before = stable_state(&tree);
        let hit_points = targets
            .iter()
            .map(|rect| Point::new(rect.x + 1.0, rect.y + rect.height * 0.5))
            .collect::<Vec<_>>();
        let hits = |tree: &WidgetTree| {
            hit_points
                .iter()
                .map(|pos| {
                    let hit = tree.hit_test(pos.x, pos.y).unwrap();
                    (hit.widget_id, hit.local_pos, hit.absolute_bounds)
                })
                .collect::<Vec<_>>()
        };
        let hits_before = hits(&tree);
        assert!(hits_before.iter().all(|hit| hit.0 == owners[1]));
        let selected = ordinary
            .0
            .iter()
            .filter(|rect| rect.2 == LAYER_CONTENT + 1)
            .collect::<Vec<_>>();
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].0.x, targets[0].x);
        assert_eq!(selected[0].0.width, targets[0].width);
        assert_eq!(
            selected[0].0.y + selected[0].0.height,
            targets[0].y + targets[0].height
        );
        assert_eq!(selected[0].1, theme.colors.borders.focus.into());
        let check_projection = |fragment: &[SceneNode], target: Option<usize>| {
            let (clip, contents) = clipped(fragment);
            assert_eq!(clip, origin);
            let borders = contents
                .iter()
                .filter_map(|node| match node {
                    SceneNode::BorderRect(border) => Some(border),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(
                borders.len(),
                usize::from(target.is_some()),
                "current keyboard target needs one cue, including on the selected tab"
            );
            if let Some(index) = target {
                let border = borders[0];
                assert_eq!(border.rect, targets[index]);
                assert_eq!(border.border_color, theme.colors.borders.focus.into());
                assert_eq!(border.border_width, theme.focus_ring.focused_width);
                assert_eq!(border.radius, 0.0);
                assert_eq!(border.fill_color.a, 0.0);
                assert_eq!(border.z_index, LAYER_FOCUS);
                assert!(ordinary.0.iter().all(|rect| rect.2 < border.z_index));
                assert!(ordinary.1.iter().all(|label| label.4 < border.z_index));
            }
            assert_eq!(rest(fragment), ordinary);
        };
        check_projection(&baseline, None);
        assert_eq!(sink.take(), None);
        let peer_fragment = harness.local_fragment(owners[2]).unwrap();
        let root_fragment = harness.local_fragment(owners[0]).unwrap();
        let changed = |frame: &crate::core::app::RetainedSceneTestFrame,
                       harness: &RetainedSceneTestHarness,
                       previous: &Arc<[SceneNode]>,
                       composed: &Arc<[SceneNode]>| {
            assert!(frame.scene_built && !frame.needs_next_frame);
            assert!(!Arc::ptr_eq(
                previous,
                &harness.local_fragment(owners[1]).unwrap()
            ));
            let next = composition(frame);
            assert!(!Arc::ptr_eq(composed, &next));
            assert!(Arc::ptr_eq(
                &peer_fragment,
                &harness.local_fragment(owners[2]).unwrap()
            ));
            assert!(Arc::ptr_eq(
                &root_fragment,
                &harness.local_fragment(owners[0]).unwrap()
            ));
            next
        };
        let mut previous = baseline;
        let mut composed = composition(&initial);

        router.request_focus_for_widget(&mut tree, owners[1], domains.primary_state_mut());
        assert_eq!(bar(&tree, node).focus_index, 0);
        let before_paint = paint_state(&tree);
        let focused = frame(&mut tree, &mut harness, &mut text, &domains);
        let fragment = harness.local_fragment(owners[1]).unwrap();
        eprintln!(
            "framework_owner={:?} active={:?} focus_index={} first_frame_borders={}",
            domains.primary_state().focus_manager.focused(),
            bar(&tree, node).active_tab,
            bar(&tree, node).focus_index,
            clipped(&fragment)
                .1
                .iter()
                .filter(|node| matches!(node, SceneNode::BorderRect(_)))
                .count()
        );
        check_projection(&fragment, Some(0));
        assert_eq!(paint_state(&tree), before_paint);
        composed = changed(&focused, &harness, &previous, &composed);
        previous = fragment;
        let idle = frame(&mut tree, &mut harness, &mut text, &domains);
        assert!(!idle.scene_built && !idle.needs_next_frame);
        assert!(Arc::ptr_eq(
            &previous,
            &harness.local_fragment(owners[1]).unwrap()
        ));

        router.route_input_event(
            &mut tree,
            FusedInputEvent::PointerDown {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                pos: hit_points[1],
                device: PointerDevice::Mouse,
                button: PointerButton::Primary,
                click_count: 1,
            },
            Some(root),
            &mut text,
            None,
            domains.primary_state_mut(),
            &mut bindings,
        );
        assert_eq!(bar(&tree, node).focus_index, 1);
        assert_eq!(sink.take(), Some(TabId(2)));
        let before_paint = paint_state(&tree);
        let moved = frame(&mut tree, &mut harness, &mut text, &domains);
        let fragment = harness.local_fragment(owners[1]).unwrap();
        check_projection(&fragment, Some(1));
        assert_eq!(paint_state(&tree), before_paint);
        assert_eq!(bar(&tree, node).active_tab, Some(TabId(1)));
        assert_eq!(sink.take(), None);
        assert!(
            domains
                .primary_state()
                .pointer_capture_manager
                .current_capture(PointerId::MOUSE)
                .is_none()
        );
        composed = changed(&moved, &harness, &previous, &composed);
        previous = fragment;

        router.clear_focus(&mut tree, domains.primary_state_mut());
        let before_paint = paint_state(&tree);
        let lost = frame(&mut tree, &mut harness, &mut text, &domains);
        let fragment = harness.local_fragment(owners[1]).unwrap();
        check_projection(&fragment, None);
        assert_eq!(paint_state(&tree), before_paint);
        composed = changed(&lost, &harness, &previous, &composed);
        previous = fragment;

        // Publish actual framework ownership without a widget lifecycle mirror or manual dirtying.
        domains
            .primary_state_mut()
            .focus_manager
            .set_focus(Some(owners[1]));
        assert!(!tree.node(node).unwrap().dirty_paint);
        assert_eq!(bar(&tree, node).focus_index, 1);
        let before_paint = paint_state(&tree);
        let published = frame(&mut tree, &mut harness, &mut text, &domains);
        let fragment = harness.local_fragment(owners[1]).unwrap();
        check_projection(&fragment, Some(1));
        assert_eq!(paint_state(&tree), before_paint);
        composed = changed(&published, &harness, &previous, &composed);
        previous = fragment;

        let mut pointer_domains =
            InteractionDomains::with_ui_interaction_policy(UiInteractionPolicy::PointerDriven);
        router.request_focus_for_widget(&mut tree, owners[1], pointer_domains.primary_state_mut());
        assert_eq!(
            pointer_domains.primary_state().focus_manager.focused(),
            Some(owners[1])
        );
        let before_paint = paint_state(&tree);
        let pointer = frame(&mut tree, &mut harness, &mut text, &pointer_domains);
        let fragment = harness.local_fragment(owners[1]).unwrap();
        check_projection(&fragment, None);
        assert_eq!(paint_state(&tree), before_paint);
        changed(&pointer, &harness, &previous, &composed);
        // The harness does not consume the router's one-shot focus-change request.
        assert!(router.continuous_frames_requested());
        router.reset_continuous_frames_request();
        let idle = frame(&mut tree, &mut harness, &mut text, &pointer_domains);
        assert!(!idle.scene_built && !idle.visual_changed && !idle.needs_next_frame);
        assert!(Arc::ptr_eq(
            &fragment,
            &harness.local_fragment(owners[1]).unwrap()
        ));
        assert!(!router.continuous_frames_requested());
        assert_eq!(stable_state(&tree), state_before);
        assert_eq!(geometry(&tree), geometry_before);
        assert_eq!(hits(&tree), hits_before);
        assert_eq!(sink.take(), None);
    }

    #[test]
    fn tab_bar_hover_repaints_only_changed_retained_owner() {
        fn bar(tree: &WidgetTree, node: NodeId) -> &TabBar {
            tree.node(node)
                .unwrap()
                .widget
                .as_any()
                .downcast_ref()
                .unwrap()
        }
        fn surfaces(nodes: &[SceneNode]) -> Vec<RectNode> {
            let mut result = Vec::new();
            for node in nodes {
                match node {
                    SceneNode::Clip(clip) => result.extend(surfaces(&clip.children)),
                    SceneNode::RetainedFragment(fragment) => {
                        result.extend(surfaces(&fragment.children))
                    }
                    SceneNode::Rect(surface) if surface.z_index == LAYER_CONTENT => {
                        result.push(surface.clone())
                    }
                    _ => {}
                }
            }
            result
        }
        fn labels(nodes: &[SceneNode]) -> Vec<(String, Point, f32)> {
            let mut result = Vec::new();
            for node in nodes {
                match node {
                    SceneNode::Clip(clip) => result.extend(labels(&clip.children)),
                    SceneNode::RetainedFragment(fragment) => {
                        result.extend(labels(&fragment.children))
                    }
                    SceneNode::Text(label) => {
                        result.push((label.text.clone(), label.position, label.font_size))
                    }
                    _ => {}
                }
            }
            result
        }
        fn composition(frame: &crate::core::app::RetainedSceneTestFrame) -> Arc<[SceneNode]> {
            let SceneNode::RetainedFragment(root) = &frame.scene_root[0] else {
                panic!()
            };
            root.children.clone()
        }

        let sink = tab_bar_sink();
        let mut control = TabBar::new(sink.clone());
        control.set_tabs(&[
            (TabId(1), "Active", true),
            (TabId(2), "First inactive", true),
            (TabId(3), "Second inactive", true),
            (TabId(4), "Disabled", false),
        ]);
        control.set_active_tab(Some(TabId(1)));
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(Flex::column().with_spacing(12.0)), None, 0);
        let node = tree.add_child(root, Box::new(control), None, 0);
        let peer = tree.add_child(root, Box::new(Label::new("Unchanged sibling")), None, 1);
        let owners = [root, node, peer].map(|id| tree.node(id).unwrap().id);
        let geometry = |tree: &WidgetTree| {
            [root, node, peer].map(|id| {
                let entry = tree.node(id).unwrap();
                (
                    entry.id,
                    entry.widget.id(),
                    tree.get_absolute_paint_rect(id).unwrap(),
                    entry.layout_epoch,
                )
            })
        };
        let state = |tree: &WidgetTree| {
            let value = bar(tree, node);
            (
                value.tabs.clone(),
                value.active_tab,
                value.focus_index,
                value.disabled,
                value.tab_rects.clone(),
                value.scroll_x,
                value.content_width,
                value.viewport_width,
            )
        };
        let theme = Theme::dark();
        let mut text = FontManagerTextEngine::new();
        let icons = crate::core::icons::builtin_registry();
        let mut domains = InteractionDomains::new();
        let mut router = EventRouter::new();
        let mut bindings = BindingRegistry::new();
        let mut harness = RetainedSceneTestHarness::new();
        let frame = |tree: &mut WidgetTree,
                     harness: &mut RetainedSceneTestHarness,
                     text: &mut FontManagerTextEngine,
                     domains: &InteractionDomains| {
            harness.frame_with_appearance(
                tree,
                root,
                Constraints::tight(Size::new(700.0, 120.0)),
                &theme,
                text,
                &icons,
                domains,
                WindowId::PRIMARY,
            )
        };
        let initial = frame(&mut tree, &mut harness, &mut text, &domains);
        assert!(initial.scene_built && !initial.needs_next_frame);
        let geometry_before = geometry(&tree);
        let state_before = state(&tree);
        let origin = tree.get_absolute_paint_rect(node).unwrap();
        let rects = bar(&tree, node).tab_rects.clone();
        assert!(rects[3].x + rects[3].width <= origin.width);
        let baseline = harness.local_fragment(owners[1]).unwrap();
        let label_geometry = labels(&baseline);
        let peer_fragment = harness.local_fragment(owners[2]).unwrap();
        let root_fragment = harness.local_fragment(owners[0]).unwrap();
        let check_visual = |fragment: &[SceneNode], hovered: Option<usize>| {
            assert!(
                fragment
                    .iter()
                    .any(|node| matches!(node, SceneNode::Clip(clip) if clip.rect == origin))
            );
            let actual = surfaces(fragment);
            assert_eq!(actual.len(), rects.len());
            for (index, surface) in actual.iter().enumerate() {
                let tab = rects[index];
                assert_eq!(
                    surface.rect,
                    RectF::new(origin.x + tab.x, origin.y + tab.y, tab.width, tab.height)
                );
                let color = if index == 0 {
                    theme.colors.controls.background_pressed
                } else if hovered == Some(index) && index != 3 {
                    theme.colors.controls.background_hover
                } else {
                    theme.colors.controls.background
                };
                assert_eq!(
                    surface.color,
                    color.into(),
                    "retained tab {index} must show current hover {hovered:?}"
                );
            }
            assert_eq!(labels(fragment), label_geometry);
        };
        check_visual(&baseline, None);
        let time = Instant::now();
        let route = |pos: Point,
                     tree: &mut WidgetTree,
                     router: &mut EventRouter,
                     text: &mut FontManagerTextEngine,
                     domains: &mut InteractionDomains,
                     bindings: &mut BindingRegistry| {
            router.route_input_event(
                tree,
                FusedInputEvent::PointerMove {
                    window_id: WindowId::PRIMARY,
                    time,
                    pos,
                    delta: (0.0, 0.0),
                    device: PointerDevice::Mouse,
                    buttons: PointerButtonsState {
                        primary: false,
                        secondary: false,
                        auxiliary: false,
                    },
                },
                Some(root),
                text,
                None,
                domains.primary_state_mut(),
                bindings,
            )
        };
        let point = |index: usize, inset: f32| {
            Point::new(
                origin.x + rects[index].x + inset,
                origin.y + rects[index].height * 0.5,
            )
        };
        router.reset_continuous_frames_request();
        route(
            point(1, 2.0),
            &mut tree,
            &mut router,
            &mut text,
            &mut domains,
            &mut bindings,
        );
        let entry_requested_frames = router.continuous_frames_requested();
        assert_eq!(domains.primary_state().hovered_widget, Some(owners[1]));
        let entered = frame(&mut tree, &mut harness, &mut text, &domains);
        assert!(entered.scene_built && !entered.needs_next_frame);
        let mut previous = harness.local_fragment(owners[1]).unwrap();
        check_visual(&previous, Some(1));
        let mut composed = composition(&entered);
        let idle = frame(&mut tree, &mut harness, &mut text, &domains);
        assert!(!idle.scene_built && !idle.needs_next_frame);

        for (pos, expected, changed) in [
            (point(2, 2.0), Some(2), true),
            (point(2, rects[2].width - 2.0), Some(2), false),
            (Point::new(origin.x - 2.0, origin.y - 2.0), None, true),
        ] {
            router.reset_continuous_frames_request();
            let old_owner = domains.primary_state().hovered_widget;
            route(
                pos,
                &mut tree,
                &mut router,
                &mut text,
                &mut domains,
                &mut bindings,
            );
            let dirty = tree.node(node).unwrap().dirty_paint;
            let ancestor_dirty = tree.node(root).unwrap().dirty_composition;
            let requested_frames = router.continuous_frames_requested();
            assert_eq!(bar(&tree, node).hovered_index, expected);
            assert_eq!(
                domains.primary_state().hovered_widget,
                expected.map(|_| owners[1])
            );
            if expected.is_some() {
                assert_eq!(old_owner, Some(owners[1]));
            }
            let current = frame(&mut tree, &mut harness, &mut text, &domains);
            let fragment = harness.local_fragment(owners[1]).unwrap();
            eprintln!(
                "hover={expected:?} same_owner={} owner_dirty={dirty} ancestor_dirty={ancestor_dirty} scene_built={} local_reused={} router_frames={requested_frames}",
                old_owner == domains.primary_state().hovered_widget,
                current.scene_built,
                Arc::ptr_eq(&previous, &fragment)
            );
            // Assert the actual retained visual first, so the pre-fix failure isolates staleness.
            check_visual(&fragment, expected);
            assert_eq!(dirty, changed);
            assert_eq!(ancestor_dirty, changed);
            assert_eq!(current.scene_built, changed);
            assert!(!current.needs_next_frame && !requested_frames && !entry_requested_frames);
            if changed {
                assert!(!Arc::ptr_eq(&previous, &fragment));
                let next_composed = composition(&current);
                assert!(!Arc::ptr_eq(&composed, &next_composed));
                composed = next_composed;
            } else {
                assert!(Arc::ptr_eq(&previous, &fragment));
                assert!(!current.visual_changed);
            }
            assert!(Arc::ptr_eq(
                &peer_fragment,
                &harness.local_fragment(owners[2]).unwrap()
            ));
            assert!(Arc::ptr_eq(
                &root_fragment,
                &harness.local_fragment(owners[0]).unwrap()
            ));
            assert_eq!(geometry(&tree), geometry_before);
            assert_eq!(state(&tree), state_before);
            assert_eq!(sink.get(), None);
            assert_eq!(domains.primary_state().focus_manager.focused(), None);
            assert!(
                domains
                    .primary_state()
                    .pointer_capture_manager
                    .current_capture(PointerId::MOUSE)
                    .is_none()
            );
            previous = fragment;
        }
        let idle = frame(&mut tree, &mut harness, &mut text, &domains);
        assert!(!idle.scene_built && !idle.visual_changed && !idle.needs_next_frame);
        assert!(Arc::ptr_eq(
            &previous,
            &harness.local_fragment(owners[1]).unwrap()
        ));
        assert!(!router.continuous_frames_requested());

        // An already-empty direct leave is independently a no-op.
        let tree_ptr = &mut tree as *mut WidgetTree;
        let mut cx = EventContext::new(
            owners[1],
            tree_ptr,
            0,
            std::ptr::null_mut(),
            None,
            None,
            false,
            false,
            domains.primary_state().keyboard_mode,
        );
        tree.node_mut(node).unwrap().widget.on_hover_leave(&mut cx);
        assert!(!cx.continuous_frames_requested());
        assert!(
            !tree.node(node).unwrap().dirty_paint && !tree.node(root).unwrap().dirty_composition
        );
        assert_eq!(sink.get(), None);
    }

    #[test]
    fn tab_bar_keyboard_activation_requests_frames_only_for_selection_intent() {
        use crate::core::event::{Modifiers, PhysicalKeyCode};

        fn bar(tree: &WidgetTree, node: NodeId) -> &TabBar {
            tree.node(node)
                .unwrap()
                .widget
                .as_any()
                .downcast_ref()
                .unwrap()
        }
        fn bar_mut(tree: &mut WidgetTree, node: NodeId) -> &mut TabBar {
            tree.node_mut(node)
                .unwrap()
                .widget
                .as_any_mut()
                .downcast_mut()
                .unwrap()
        }
        let tabs = [
            (TabId(1), "Active", true),
            (TabId(2), "Other", true),
            (TabId(3), "Disabled", false),
        ];
        let sink = tab_bar_sink();
        let mut control = TabBar::new(sink.clone());
        control.set_tabs(&tabs);
        control.set_active_tab(Some(TabId(1)));
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(Flex::column().with_spacing(12.0)), None, 0);
        let node = tree.add_child(root, Box::new(control), None, 0);
        let peer = tree.add_child(root, Box::new(Label::new("Unchanged peer")), None, 1);
        let [root_id, owner, peer_id] = [root, node, peer].map(|id| tree.node(id).unwrap().id);
        let geometry = |tree: &WidgetTree| {
            [root, node, peer].map(|id| {
                let entry = tree.node(id).unwrap();
                (
                    entry.id,
                    entry.widget.id(),
                    tree.get_absolute_paint_rect(id).unwrap(),
                    entry.layout_epoch,
                )
            })
        };
        let projection = |tree: &WidgetTree| {
            let value = bar(tree, node);
            (
                value.active_tab,
                value.focus_index,
                value.tabs.clone(),
                value.tab_rects.clone(),
                value.scroll_x,
                value.content_width,
                value.viewport_width,
                value.hovered_index,
                value.disabled,
            )
        };
        let theme = Theme::default();
        let icons = crate::core::icons::builtin_registry();
        let mut text = FontManagerTextEngine::new();
        let mut harness = RetainedSceneTestHarness::new();
        let mut router = EventRouter::new();
        let mut domains = InteractionDomains::new();
        let mut bindings = BindingRegistry::new();
        let frame = |tree: &mut WidgetTree,
                     harness: &mut RetainedSceneTestHarness,
                     text: &mut FontManagerTextEngine,
                     domains: &InteractionDomains| {
            harness.frame_with_appearance(
                tree,
                root,
                Constraints::tight(Size::new(400.0, 120.0)),
                &theme,
                text,
                &icons,
                domains,
                WindowId::PRIMARY,
            )
        };
        frame(&mut tree, &mut harness, &mut text, &domains);
        let bounds = geometry(&tree);
        let origin = bounds[1].2;
        let hit = |tree: &WidgetTree| {
            let hit = tree.hit_test(origin.x + 1.0, origin.y + 1.0).unwrap();
            (hit.widget_id, hit.local_pos, hit.absolute_bounds)
        };
        let original_hit = hit(&tree);
        router.request_focus_for_widget(&mut tree, owner, domains.primary_state_mut());
        assert!(router.continuous_frames_requested());
        let focused = frame(&mut tree, &mut harness, &mut text, &domains);
        assert!(focused.scene_built && !focused.needs_next_frame);
        let settled = frame(&mut tree, &mut harness, &mut text, &domains);
        assert!(!settled.scene_built && !settled.needs_next_frame);
        router.reset_continuous_frames_request();
        assert_eq!(domains.primary_state().focus_manager.focused(), Some(owner));
        assert_eq!(
            (bar(&tree, node).active_tab, bar(&tree, node).focus_index),
            (Some(TabId(1)), 0)
        );
        let root_fragment = harness.local_fragment(root_id).unwrap();
        let peer_fragment = harness.local_fragment(peer_id).unwrap();
        let active_fragment = harness.local_fragment(owner).unwrap();
        let before = projection(&tree);
        assert!(router.activate_focused_widget(&mut tree, domains.primary_state_mut(), None));
        assert_eq!(sink.get(), None);
        assert!(
            !router.continuous_frames_requested(),
            "already-active keyboard activation must not request a frame without selection intent"
        );
        assert_eq!(projection(&tree), before);
        assert!(!tree.node(node).unwrap().dirty_paint);
        assert!(!tree.node(node).unwrap().dirty_layout);
        assert!(!tree.node(root).unwrap().dirty_composition);
        let noop = frame(&mut tree, &mut harness, &mut text, &domains);
        assert!(!noop.scene_built && !noop.visual_changed && !noop.needs_next_frame);
        assert!(Arc::ptr_eq(
            &active_fragment,
            &harness.local_fragment(owner).unwrap()
        ));

        router.route_input_event(
            &mut tree,
            FusedInputEvent::KeyDown {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                key_code: KeyCode::Right,
                physical_key: PhysicalKeyCode::ArrowRight,
                reservation: None,
                modifiers: Modifiers::default(),
                repeat: false,
            },
            Some(root),
            &mut text,
            None,
            domains.primary_state_mut(),
            &mut bindings,
        );
        assert_eq!(bar(&tree, node).focus_index, 1);
        assert_eq!(sink.get(), None);
        assert!(!tree.node(node).unwrap().dirty_layout);
        let navigation = frame(&mut tree, &mut harness, &mut text, &domains);
        assert!(navigation.scene_built && !navigation.needs_next_frame);
        let target_fragment = harness.local_fragment(owner).unwrap();
        assert!(!Arc::ptr_eq(&active_fragment, &target_fragment));
        let settled = frame(&mut tree, &mut harness, &mut text, &domains);
        assert!(!settled.scene_built && !settled.needs_next_frame);
        router.reset_continuous_frames_request();
        let before = projection(&tree);
        assert!(router.activate_focused_widget(&mut tree, domains.primary_state_mut(), None));
        assert_eq!(sink.get(), Some(TabId(2)));
        assert!(router.continuous_frames_requested());
        assert_eq!(projection(&tree), before);
        assert_eq!(bar(&tree, node).active_tab, Some(TabId(1)));
        assert!(!tree.node(node).unwrap().dirty_paint);
        assert!(!tree.node(node).unwrap().dirty_layout);
        assert!(!tree.node(root).unwrap().dirty_composition);
        router.reset_continuous_frames_request();
        let intent_only = frame(&mut tree, &mut harness, &mut text, &domains);
        assert!(
            !intent_only.scene_built
                && !intent_only.visual_changed
                && !intent_only.needs_next_frame
        );
        assert!(Arc::ptr_eq(
            &target_fragment,
            &harness.local_fragment(owner).unwrap()
        ));

        // Return to active content with a real pending external intent, then revisit the other target.
        for (key_code, physical_key, expected) in [
            (KeyCode::Left, PhysicalKeyCode::ArrowLeft, 0),
            (KeyCode::Right, PhysicalKeyCode::ArrowRight, 1),
        ] {
            router.route_input_event(
                &mut tree,
                FusedInputEvent::KeyDown {
                    window_id: WindowId::PRIMARY,
                    time: Instant::now(),
                    key_code,
                    physical_key,
                    reservation: None,
                    modifiers: Modifiers::default(),
                    repeat: false,
                },
                Some(root),
                &mut text,
                None,
                domains.primary_state_mut(),
                &mut bindings,
            );
            assert_eq!(bar(&tree, node).focus_index, expected);
            assert!(!tree.node(node).unwrap().dirty_layout);
            let navigation = frame(&mut tree, &mut harness, &mut text, &domains);
            assert!(navigation.scene_built && !navigation.needs_next_frame);
            let settled = frame(&mut tree, &mut harness, &mut text, &domains);
            assert!(!settled.scene_built && !settled.needs_next_frame);
            router.reset_continuous_frames_request();
            if expected == 0 {
                let before = projection(&tree);
                assert!(router.activate_focused_widget(
                    &mut tree,
                    domains.primary_state_mut(),
                    None
                ));
                assert!(!router.continuous_frames_requested());
                assert_eq!(projection(&tree), before);
                assert_eq!(sink.take(), Some(TabId(2)));
                assert_eq!(sink.take(), None);
            } else {
                // Same identities/labels: observe callback eligibility, not replacement/layout policy.
                let mut unavailable = tabs;
                unavailable[1].2 = false;
                bar_mut(&mut tree, node).set_tabs(&unavailable);
                let before = projection(&tree);
                assert!(router.activate_focused_widget(
                    &mut tree,
                    domains.primary_state_mut(),
                    None
                ));
                assert_eq!(sink.take(), None);
                assert!(!router.continuous_frames_requested());
                assert_eq!(projection(&tree), before);
                bar_mut(&mut tree, node).set_tabs(&tabs);
            }
            assert_eq!(geometry(&tree), bounds);
            assert_eq!(hit(&tree), original_hit);
            assert_eq!(domains.primary_state().focus_manager.focused(), Some(owner));
            assert!(
                domains
                    .primary_state()
                    .pointer_capture_manager
                    .current_capture(PointerId::MOUSE)
                    .is_none()
            );
            assert!(!tree.node(node).unwrap().dirty_paint);
            assert!(!tree.node(node).unwrap().dirty_layout);
            assert!(!tree.node(root).unwrap().dirty_composition);
            assert!(Arc::ptr_eq(
                &peer_fragment,
                &harness.local_fragment(peer_id).unwrap()
            ));
            assert!(Arc::ptr_eq(
                &root_fragment,
                &harness.local_fragment(root_id).unwrap()
            ));
        }
        let final_owner = harness.local_fragment(owner).unwrap();
        let idle = frame(&mut tree, &mut harness, &mut text, &domains);
        assert!(!idle.scene_built && !idle.visual_changed && !idle.needs_next_frame);
        assert!(Arc::ptr_eq(
            &final_owner,
            &harness.local_fragment(owner).unwrap()
        ));
        assert!(!router.continuous_frames_requested());
        assert_eq!(sink.take(), None);
        assert_eq!(bar(&tree, node).active_tab, Some(TabId(1)));
    }
}
