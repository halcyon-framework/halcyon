//! Tap the controls to edit or select. Drag from the same controls to scroll.

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::appearance::ButtonMaterialPolicy;
use halcyon::core::event::FusedInputEvent;
use halcyon::core::layout::{EdgeInsets, FlexParams};
use halcyon::core::widget::Widget;
use halcyon::core::widget_tree::{NodeId, WidgetTree};
use halcyon::prelude::{Button, Flex, Label, ScrollArea, TextField};
use halcyon::widgets::{
    checkbox::Checkbox, combo_box::ComboBox, slider::Slider, spin_box::SpinBox, switch::Switch,
    text_area::TextArea,
};
use std::cell::Cell;
use std::rc::Rc;

#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
fn main() {
    run_touch_input();
}

pub(crate) fn run_touch_input() {
    let mut config = AppConfig::new("Touch Input").with_size(390.0, 640.0);
    config.theme.button_material_policy = ButtonMaterialPolicy::SdfV3;
    App::new(config).run(TouchInput {
        placeholder: Label::new(""),
        rebuild: true,
        tap_count: Rc::new(Cell::new(0)),
        tap_status: None,
        displayed_taps: 0,
    });
}

struct TouchInput {
    placeholder: Label,
    rebuild: bool,
    tap_count: Rc<Cell<u64>>,
    tap_status: Option<NodeId>,
    displayed_taps: u64,
}

impl Screen for TouchInput {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        let scroll = cx.mount_child(Box::new(ScrollArea::new_unwired()));
        let body = cx.tree.add_child(
            scroll,
            Box::new(
                Flex::column()
                    .with_padding(EdgeInsets::all(20.0))
                    .with_spacing(12.0),
            ),
            None,
            0,
        );
        cx.tree
            .node_mut(scroll)
            .unwrap()
            .widget_mut()
            .as_any_mut()
            .downcast_mut::<ScrollArea>()
            .unwrap()
            .wire_content(body);

        let mut options = ComboBox::simple((1..=24).map(|i| format!("Option {i}")).collect());
        options.set_text_input_enabled(false);
        let tap_count = Rc::clone(&self.tap_count);
        let tap_button = Button::with_text("Test tap").on_click(move || {
            let next = tap_count.get().checked_add(1).expect("tap count overflow");
            tap_count.set(next);
        });
        let controls: [(Box<dyn Widget>, f32); 16] = [
            (Box::new(Label::new("Touch Input").with_size(25.0)), 34.0),
            (
                Box::new(Label::new("Tap to edit or choose. Swipe to scroll.")),
                26.0,
            ),
            (Box::new(Label::new("Email")), 23.0),
            (
                Box::new(TextField::new().with_placeholder("name@example.com")),
                38.0,
            ),
            (Box::new(Label::new("Profile")), 23.0),
            (Box::new(options), 38.0),
            (
                Box::new(Checkbox::new().with_label("Send me updates")),
                35.0,
            ),
            (Box::new(Label::new("Notes")), 23.0),
            (
                Box::new(TextArea::new().with_placeholder("Try Enter, emoji, or IME")),
                116.0,
            ),
            (Box::new(Label::new("Volume")), 23.0),
            (Box::new(Slider::new(0.0, 100.0).with_value(30.0)), 37.0),
            (Box::new(Label::new("Quantity")), 23.0),
            (Box::new(SpinBox::new().with_value(5)), 38.0),
            (
                Box::new(Switch::new().with_label("Include notifications")),
                36.0,
            ),
            (
                Box::new(Label::new("Scroll to either end. Touch to stop a flick.")),
                26.0,
            ),
            (Box::new(tap_button), 42.0),
        ];
        for (widget, height) in controls {
            let child = cx.tree.add_child(body, widget, None, 0);
            cx.tree.set_flex(child, FlexParams::fixed(height));
        }
        self.displayed_taps = self.tap_count.get();
        let status = cx.tree.add_child(
            body,
            Box::new(Label::new(format!("Test tap count: {}", self.displayed_taps))),
            None,
            0,
        );
        cx.tree.set_flex(status, FlexParams::fixed(23.0));
        self.tap_status = Some(status);
        &mut self.placeholder
    }

    fn handle_input(&mut self, _event: &FusedInputEvent, _cx: &mut ScreenContext<'_>) -> bool {
        false
    }

    fn update(&mut self, tree: &mut WidgetTree) {
        let count = self.tap_count.get();
        if count == self.displayed_taps {
            return;
        }
        let Some(status) = self.tap_status else {
            return;
        };
        tree.node_mut(status)
            .expect("current tap status mount")
            .widget_mut()
            .as_any_mut()
            .downcast_mut::<Label>()
            .expect("tap status Label")
            .set_text(format!("Test tap count: {count}"));
        tree.mark_layout_dirty(status);
        self.displayed_taps = count;
    }

    fn mark_rebuild(&mut self) {
        self.rebuild = true;
    }

    fn take_rebuild_flag(&mut self) -> bool {
        std::mem::take(&mut self.rebuild)
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use halcyon::core::{
        app::{AppContext, EngineTicker, layout_tree},
        event::{
            GestureKind, InteractionDomainState, KeyboardAuthorityMode, RawInputEvent, TouchPhase,
            WindowId,
        },
        event_router::EventRouter,
        input_fusion::InputFusion,
        layout::Constraints,
        math::{Point, Size},
        rect_authority::{LayoutAuthorityGuard, UpdateAuthorityGuard},
        text_engine::TextEngine,
        theme::Theme,
    };
    use halcyon::platform::font_manager_text_engine::FontManagerTextEngine;
    use halcyon::runtime::binding_registry::BindingRegistry;
    use halcyon::widgets::screen_host::ScreenHost;
    use std::time::Instant;

    fn scroll_area(tree: &WidgetTree, node: NodeId) -> &ScrollArea {
        tree.node(node)
            .unwrap()
            .widget()
            .as_any()
            .downcast_ref::<ScrollArea>()
            .unwrap()
    }

    #[test]
    fn mounted_touch_input_distinguishes_tap_from_pan() {
        let theme = Theme::default();
        let mut context = AppContext::new(theme.clone());
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(ScreenHost::new()), None, 0);
        let mut screen = TouchInput {
            placeholder: Label::new(""),
            rebuild: true,
            tap_count: Rc::new(Cell::new(0)),
            tap_status: None,
            displayed_taps: 0,
        };
        screen.build(&mut ScreenFrameContext {
            app: &mut context,
            tree: &mut tree,
            parent_node: root,
        });

        let mut text_engine = FontManagerTextEngine::new_with_system_ui();
        let mut ticker = EngineTicker::new(Instant::now());
        let mut layout = |tree: &mut WidgetTree, text_engine: &mut dyn TextEngine| {
            let frame = ticker.begin_frame();
            {
                let _update = UpdateAuthorityGuard::enter();
                tree.advance_kinetic_scroll_for_frame(&frame);
            }
            let _layout = LayoutAuthorityGuard::enter();
            layout_tree(
                tree,
                root,
                Constraints::tight(Size::new(390.0, 640.0)),
                &theme,
                text_engine,
                None,
            );
        };
        layout(&mut tree, &mut text_engine);

        let scroll = tree.children(root)[0];
        let body = scroll_area(&tree, scroll).content();
        let controls = tree.children(body);
        assert_eq!(controls.len(), 17);
        let field = controls[3];
        let button = controls[15];
        let status = controls[16];
        assert_eq!(screen.tap_status, Some(status));
        assert!(
            tree.node(field)
                .unwrap()
                .widget()
                .as_any()
                .is::<TextField>()
        );
        assert!(tree.node(button).unwrap().widget().as_any().is::<Button>());
        let field_owner = tree.widget_id_for_node(field).unwrap();
        let button_owner = tree.widget_id_for_node(button).unwrap();
        let scroll_owner = tree.widget_id_for_node(scroll).unwrap();
        let viewport = tree.get_viewport(body).unwrap();
        assert!(viewport.height > 0.0);
        assert!(scroll_area(&tree, scroll).content_extent_height() > viewport.height);
        let button_rect = tree.get_absolute_paint_rect(button).unwrap();
        assert!(button_rect.y >= viewport.bottom());

        let visible_center = |tree: &WidgetTree, node| {
            let rect = tree.get_absolute_paint_rect(node).unwrap();
            Point::new(
                rect.x + rect.width / 2.0,
                rect.y + rect.height / 2.0 - scroll_area(tree, scroll).visual_scroll_y(),
            )
        };
        let mut domain = InteractionDomainState::new();
        let mut router = EventRouter::new();
        let mut fusion = InputFusion::new();
        let mut bindings = BindingRegistry::new();
        let mut touch =
            |tree: &mut WidgetTree, text_engine: &mut dyn TextEngine, contact_id, phase, pos| {
                let mut events = Vec::new();
                fusion.process_raw_with_gesture_sessions(
                    RawInputEvent::Touch {
                        window_id: WindowId::PRIMARY,
                        time: Instant::now(),
                        contact_id,
                        phase,
                        pos,
                    },
                    domain.focus_manager.interaction_domain_id(),
                    &mut domain.gesture_registry,
                    tree,
                    None,
                    &mut events,
                );
                fusion.flush_with_gesture_sessions(&mut events);
                let pointer = events
                    .iter()
                    .find_map(|event| match event.event() {
                        FusedInputEvent::TouchDown { pointer_id, .. }
                        | FusedInputEvent::TouchMove { pointer_id, .. }
                        | FusedInputEvent::TouchUp { pointer_id, .. } => Some(*pointer_id),
                        _ => None,
                    })
                    .expect("qualified touch event");
                for event in events {
                    router.route_session_input_event(
                        tree,
                        event,
                        Some(root),
                        text_engine,
                        None,
                        &mut domain,
                        &mut bindings,
                    );
                    assert_eq!(domain.keyboard_mode, KeyboardAuthorityMode::Navigation);
                    assert_ne!(domain.focus_manager.focused(), Some(field_owner));
                }
                let winner = domain
                    .gesture_registry
                    .committed_candidate(pointer)
                    .map(|candidate| (candidate.widget_id, candidate.kind));
                (
                    domain.pointer_capture_manager.current_capture(pointer),
                    winner,
                )
            };

        let start = visible_center(&tree, field);
        assert_eq!(
            tree.hit_test(start.x, start.y).unwrap().widget_id,
            field_owner
        );
        let before = scroll_area(&tree, scroll).visual_scroll_y();
        let displacement = visible_center(&tree, button).y - (viewport.bottom() - 16.0);
        let end = Point::new(start.x, start.y - displacement);
        assert!(displacement > 4.0 && viewport.contains(end));
        let pan = (
            Some(scroll_owner),
            Some((scroll_owner, GestureKind::Scroll)),
        );
        touch(&mut tree, &mut text_engine, 1, TouchPhase::Started, start);
        assert_eq!(
            touch(&mut tree, &mut text_engine, 1, TouchPhase::Moved, end),
            pan
        );
        assert!(scroll_area(&tree, scroll).visual_scroll_y() > before);
        assert_eq!(
            touch(&mut tree, &mut text_engine, 1, TouchPhase::Ended, end),
            (None, None)
        );
        assert_eq!(screen.tap_count.get(), 0);

        layout(&mut tree, &mut text_engine);
        for contact in [2, 3] {
            let at = visible_center(&tree, button);
            assert!(viewport.contains(at));
            assert_eq!(tree.hit_test(at.x, at.y).unwrap().widget_id, button_owner);
            touch(
                &mut tree,
                &mut text_engine,
                contact,
                TouchPhase::Started,
                at,
            );
            touch(&mut tree, &mut text_engine, contact, TouchPhase::Ended, at);
            assert_eq!(screen.tap_count.get(), contact - 1);
        }
        assert_eq!(screen.displayed_taps, 0);

        let start = visible_center(&tree, button);
        assert_eq!(
            tree.hit_test(start.x, start.y).unwrap().widget_id,
            button_owner
        );
        let end = Point::new(start.x, start.y + 8.0);
        assert!(viewport.contains(end));
        let before = scroll_area(&tree, scroll).visual_scroll_y();
        assert!(before >= 8.0);
        assert_eq!(
            touch(&mut tree, &mut text_engine, 4, TouchPhase::Started, start),
            (Some(button_owner), None)
        );
        assert_eq!(
            touch(&mut tree, &mut text_engine, 4, TouchPhase::Moved, end),
            pan
        );
        assert!(scroll_area(&tree, scroll).visual_scroll_y() < before);
        assert_eq!(
            touch(&mut tree, &mut text_engine, 4, TouchPhase::Ended, end),
            (None, None)
        );
        assert_eq!(screen.tap_count.get(), 2);

        let _update = UpdateAuthorityGuard::enter();
        screen.update(&mut tree);
        assert_eq!(screen.displayed_taps, 2);
        assert_eq!(screen.tap_status, Some(status));
        let label = tree
            .node(status)
            .unwrap()
            .widget()
            .as_any()
            .downcast_ref::<Label>()
            .unwrap();
        assert_eq!(label.text_info().unwrap().0, "Test tap count: 2");
        assert_eq!(tree.widget_id_for_node(field), Some(field_owner));
        assert_eq!(tree.widget_id_for_node(button), Some(button_owner));
    }
}
