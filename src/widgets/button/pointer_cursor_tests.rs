use std::cell::Cell;
use std::rc::Rc;
use web_time::Instant;

use super::{Button, ButtonMaterialSelection};
use crate::core::app::RetainedSceneTestHarness;
use crate::core::event::{
    CursorIcon, FusedInputEvent, InteractionDomains, KeyCode, Modifiers, PhysicalKeyCode,
    PointerButton, PointerButtonsState, PointerDevice, PointerId, WindowId,
};
use crate::core::event_router::EventRouter;
use crate::core::icon::IconRegistry;
use crate::core::layout::Constraints;
use crate::core::math::{Point, Size};
use crate::core::theme::Theme;
use crate::core::widget_tree::{NodeId, WidgetTree};
use crate::platform::font_manager_text_engine::FontManagerTextEngine;
use crate::runtime::binding_registry::BindingRegistry;
use crate::widgets::{flex::Flex, label::Label, text_field::TextField};

struct Fixture {
    tree: WidgetTree,
    root: NodeId,
    text_button: NodeId,
    child_button: NodeId,
    caption: NodeId,
    unconfigured: NodeId,
    disabled: NodeId,
    text_field: NodeId,
    clicks: Rc<Cell<usize>>,
    router: EventRouter,
    domains: InteractionDomains,
    text: FontManagerTextEngine,
    bindings: BindingRegistry,
}

impl Fixture {
    fn new() -> Self {
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(Flex::column().with_spacing(8.0)), None, 0);
        let clicks = Rc::new(Cell::new(0));
        let on_click = {
            let clicks = clicks.clone();
            move || clicks.set(clicks.get() + 1)
        };
        let text_button = tree.add_child(
            root,
            Box::new(
                Button::with_text("Run")
                    .with_pointer_cursor(true)
                    .on_click(on_click.clone()),
            ),
            None,
            0,
        );
        let child_button = tree.add_child(root, Box::new(Label::new("")), None, 1);
        let caption = tree.add_child(child_button, Box::new(Label::new("Caption")), None, 0);
        crate::replace_widget!(
            tree,
            child_button,
            Button::new(caption).with_pointer_cursor(true).on_click(on_click)
        );
        let unconfigured = tree.add_child(root, Box::new(Button::with_text("Default")), None, 2);
        let disabled = tree.add_child(
            root,
            Box::new(
                Button::with_text("Disabled")
                    .with_pointer_cursor(true)
                    .with_enabled(false),
            ),
            None,
            3,
        );
        let text_field = tree.add_child(root, Box::new(TextField::new().with_text("Edit")), None, 4);
        let domains = InteractionDomains::new();
        let mut text = FontManagerTextEngine::new();
        RetainedSceneTestHarness::new().frame_with_appearance(
            &mut tree,
            root,
            Constraints::loose(Size::new(360.0, 320.0)),
            &Theme::dark(),
            &mut text,
            &IconRegistry::new(&[]),
            &domains,
            WindowId::PRIMARY,
        );
        Self {
            tree,
            root,
            text_button,
            child_button,
            caption,
            unconfigured,
            disabled,
            text_field,
            clicks,
            router: EventRouter::new(),
            domains,
            text,
            bindings: BindingRegistry::new(),
        }
    }

    fn center(&self, node: NodeId) -> Point {
        let rect = self.tree.get_absolute_paint_rect(node).unwrap();
        assert!(rect.width > 0.0 && rect.height > 0.0);
        Point::new(rect.x + rect.width * 0.5, rect.y + rect.height * 0.5)
    }

    fn route(&mut self, event: FusedInputEvent) -> Option<CursorIcon> {
        self.router.route_input_event(
            &mut self.tree,
            event,
            Some(self.root),
            &mut self.text,
            None,
            self.domains.primary_state_mut(),
            &mut self.bindings,
        );
        self.router.take_requested_cursor()
    }

    fn move_to(&mut self, pos: Point, primary: bool) -> Option<CursorIcon> {
        self.route(FusedInputEvent::PointerMove {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            pos,
            delta: (1.0, 0.0),
            device: PointerDevice::Mouse,
            buttons: PointerButtonsState {
                primary,
                ..Default::default()
            },
        })
    }

    fn down(&mut self, pos: Point, button: PointerButton, click_count: u8) -> Option<CursorIcon> {
        self.route(FusedInputEvent::PointerDown {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            pos,
            device: PointerDevice::Mouse,
            button,
            click_count,
        })
    }

    fn up(&mut self, pos: Point) -> Option<CursorIcon> {
        self.route(FusedInputEvent::PointerUp {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            pos,
            device: PointerDevice::Mouse,
            button: PointerButton::Primary,
        })
    }
}

#[test]
fn button_pointer_cursor_routes_text_and_child_caption() {
    let mut f = Fixture::new();
    for (node, point) in [
        (f.text_button, f.center(f.text_button)),
        (f.child_button, f.center(f.caption)),
    ] {
        assert_eq!(f.move_to(point, false), Some(CursorIcon::Pointer));
        assert_eq!(f.move_to(Point::new(point.x + 1.0, point.y), false), Some(CursorIcon::Pointer));
        assert_eq!(f.down(point, PointerButton::Secondary, 1), Some(CursorIcon::Pointer));
        let before = f.clicks.get();
        for count in [1, 2, 3] {
            assert_eq!(f.down(point, PointerButton::Primary, count), Some(CursorIcon::Pointer));
            assert_eq!(f.up(point), Some(CursorIcon::Pointer));
            assert_eq!(f.clicks.get(), before + usize::from(count));
        }
        let button = f.tree.node(node).unwrap().widget().as_any().downcast_ref::<Button>().unwrap();
        assert!(button.material_selection == ButtonMaterialSelection::Theme);
        assert!(button.resolved_appearance().is_ok());
    }
    assert_eq!(f.clicks.get(), 6);
}

#[test]
fn button_pointer_cursor_follows_capture_and_other_cursor_owners() {
    let mut f = Fixture::new();
    let inside = f.center(f.text_button);
    let other = f.center(f.text_field);
    let background = Point::new(340.0, 310.0);
    assert_eq!(f.down(inside, PointerButton::Primary, 1), Some(CursorIcon::Pointer));
    assert!(f.domains.primary_state().pointer_capture_manager.is_captured(PointerId::MOUSE));
    assert_eq!(f.move_to(other, true), None);
    assert_eq!(f.move_to(inside, true), Some(CursorIcon::Pointer));
    assert_eq!(f.up(inside), Some(CursorIcon::Pointer));
    assert_eq!(f.clicks.get(), 0, "an out-and-back press remains cancelled");
    assert!(!f.domains.primary_state().pointer_capture_manager.is_captured(PointerId::MOUSE));
    assert_eq!(f.move_to(background, false), None);
    assert_eq!(f.move_to(other, false), Some(CursorIcon::Text));
}

#[test]
fn button_pointer_cursor_respects_availability_and_input_type() {
    let mut f = Fixture::new();
    for point in [f.center(f.unconfigured), f.center(f.disabled)] {
        assert_eq!(f.move_to(point, false), None);
        assert_eq!(f.down(point, PointerButton::Primary, 1), None);
        assert_eq!(f.up(point), None);
    }
    assert_eq!(f.clicks.get(), 0);

    let point = f.center(f.text_button);
    assert_eq!(f.down(point, PointerButton::Primary, 1), Some(CursorIcon::Pointer));
    assert_eq!(f.up(point), Some(CursorIcon::Pointer));
    assert_eq!(f.clicks.get(), 1);
    assert_eq!(f.domains.primary_state().focus_manager.focused(), f.tree.widget_id_for_node(f.text_button));
    assert_eq!(f.route(FusedInputEvent::KeyDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        key_code: KeyCode::Space,
        physical_key: PhysicalKeyCode::Space,
        reservation: None,
        modifiers: Modifiers::default(),
        repeat: false,
    }), None);
    assert_eq!(f.route(FusedInputEvent::KeyUp {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        key_code: KeyCode::Space,
        physical_key: PhysicalKeyCode::Space,
        reservation: None,
        modifiers: Modifiers::default(),
    }), None);

    let before_touch = f.clicks.get();
    let touch = PointerId(7);
    assert_eq!(f.route(FusedInputEvent::TouchDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pointer_id: touch,
        pos: point,
        button: PointerButton::Primary,
        click_count: 1,
    }), None);
    assert_eq!(f.route(FusedInputEvent::TouchUp {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pointer_id: touch,
        pos: point,
        button: PointerButton::Primary,
    }), None);
    assert_eq!(f.clicks.get(), before_touch + 1, "the routed touch still activates");
}
