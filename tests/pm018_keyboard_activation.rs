use halcyon::core::event::{
    Event, EventPhase, InteractionDomainState, KeyCode, KeyEvent, Modifiers,
};
use halcyon::core::event_router::EventRouter;
use halcyon::core::widget::{EventContext, Widget};
use halcyon::core::widget_tree::{NodeId, WidgetDirty, WidgetTree};
use halcyon::model::breadcrumbs::breadcrumbs_sink;
use halcyon::model::dropdown_menu::dropdown_menu_handle;
use halcyon::model::menu_item::MenuItem;
use halcyon::widgets::breadcrumbs::{Breadcrumbs, CrumbId};
use halcyon::widgets::checkbox::Checkbox;
use halcyon::widgets::dropdown_menu::DropdownMenu;
use halcyon::widgets::section_surfaces::DisclosureSectionHeader;
use halcyon::widgets::segmented_control::{Segment, SegmentedControl};
use halcyon::widgets::switch::Switch;

struct ActivationFixture {
    tree: WidgetTree,
    root: NodeId,
    domain: InteractionDomainState,
    router: EventRouter,
}

impl ActivationFixture {
    fn new(widget: impl Widget + 'static) -> Self {
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(widget), None, 0);
        let mut domain = InteractionDomainState::new();
        domain
            .focus_manager
            .set_focus(Some(tree.node(root).unwrap().id()));
        let mut fixture = Self {
            tree,
            root,
            domain,
            router: EventRouter::new(),
        };
        fixture.clear_dirty();
        fixture
    }

    fn with_context(&mut self, f: impl FnOnce(&mut dyn Widget, &mut EventContext)) {
        let tree_ptr = &mut self.tree as *mut WidgetTree;
        let node = self.tree.node_mut(self.root).unwrap();
        let mut cx = EventContext::new(
            node.id(),
            tree_ptr,
            0,
            std::ptr::null_mut(),
            None,
            None,
            false,
            false,
            self.domain.keyboard_mode,
        );
        f(node.widget_mut(), &mut cx);
    }

    fn activate(&mut self) -> bool {
        self.router
            .activate_focused_widget(&mut self.tree, &mut self.domain, None)
    }

    fn widget<W: Widget + 'static>(&self) -> &W {
        self.tree
            .node(self.root)
            .unwrap()
            .widget()
            .as_any()
            .downcast_ref()
            .unwrap()
    }

    fn clear_dirty(&mut self) {
        let node = self.tree.node_mut(self.root).unwrap();
        node.dirty_layout = false;
        node.dirty_paint = false;
        node.dirty_composition = false;
        node.dirty_flags = WidgetDirty::Unchanged;
    }

    fn assert_paint_only(&self) {
        let node = self.tree.node(self.root).unwrap();
        assert!(node.dirty_paint);
        assert!(node.dirty_composition);
        assert!(!node.dirty_layout);
        assert_eq!(node.dirty_flags, WidgetDirty::PaintOnly);
    }

    fn assert_clean(&self) {
        let node = self.tree.node(self.root).unwrap();
        assert!(!node.dirty_paint);
        assert!(!node.dirty_composition);
        assert!(!node.dirty_layout);
        assert_eq!(node.dirty_flags, WidgetDirty::Unchanged);
    }
}

fn action() -> MenuItem {
    MenuItem::ActionRow {
        id: 1,
        label: "Open".into(),
        icon: None,
        shortcut: None,
        disabled: false,
        submenu: None,
    }
}

#[test]
fn pm018_keyboard_activation_checkbox() {
    let mut fixture = ActivationFixture::new(Checkbox::new());
    fixture.assert_clean();
    assert!(fixture.activate());
    assert!(fixture.widget::<Checkbox>().is_checked());
    fixture.assert_paint_only();
}

#[test]
fn pm018_keyboard_activation_segmented_selection_and_noop() {
    let mut fixture = ActivationFixture::new(SegmentedControl::new(
        vec![Segment::new("First"), Segment::new("Second")],
        0,
    ));
    fixture.with_context(|widget, cx| {
        widget.on_focus(cx);
        let right = Event::Key(KeyEvent {
            key_code: KeyCode::Right,

            physical_key: halcyon::core::event::PhysicalKeyCode::Unknown,
            is_down: true,
            is_repeat: false,
            modifiers: Modifiers::default(),
            text: None,
        });
        assert!(widget.handle_event(cx, &right, EventPhase::Target));
    });
    assert_eq!(fixture.widget::<SegmentedControl>().selected(), 0);
    fixture.clear_dirty();
    fixture.assert_clean();
    assert!(fixture.activate());
    assert_eq!(fixture.widget::<SegmentedControl>().selected(), 1);
    fixture.assert_paint_only();

    fixture.clear_dirty();
    assert!(fixture.activate());
    fixture.assert_clean();
}

#[test]
fn pm018_keyboard_activation_dropdown_open_and_noops() {
    let (controller, sink) = dropdown_menu_handle();
    let mut menu = DropdownMenu::new(controller.clone(), "Actions");
    menu.set_actions(vec![action()]);
    let mut fixture = ActivationFixture::new(menu);
    fixture.assert_clean();
    assert!(fixture.activate());
    assert!(controller.borrow().is_open());
    assert_eq!(sink.get(), None);
    fixture.assert_paint_only();

    fixture.clear_dirty();
    assert!(fixture.activate());
    fixture.assert_clean();

    let (controller, _) = dropdown_menu_handle();
    let mut disabled = DropdownMenu::new(controller.clone(), "Actions");
    disabled.set_actions(vec![action()]);
    disabled.set_disabled(true);
    let mut fixture = ActivationFixture::new(disabled);
    assert!(!fixture.activate());
    assert!(!controller.borrow().is_open());
    fixture.assert_clean();
}

#[test]
fn pm018_keyboard_activation_breadcrumb_dropdown_open_and_reset() {
    let navigation = breadcrumbs_sink();
    let (controller, sink) = dropdown_menu_handle();
    let mut breadcrumbs = Breadcrumbs::new(navigation.clone());
    breadcrumbs.set_crumbs(&[(CrumbId(1), "Current", true)]);
    breadcrumbs.set_current_crumb(Some(CrumbId(1)));
    breadcrumbs.enable_dropdown(controller.clone(), sink, vec![action()]);
    let mut fixture = ActivationFixture::new(breadcrumbs);
    fixture.assert_clean();
    assert!(fixture.activate());
    assert!(controller.borrow().is_open());
    assert_eq!(navigation.get(), None);
    fixture.assert_paint_only();

    controller
        .borrow_mut()
        .surface_mut()
        .set_highlighted(Some(0));
    fixture.clear_dirty();
    assert!(fixture.activate());
    assert_eq!(controller.borrow().surface().highlighted_index(), None);
    fixture.assert_paint_only();
}

#[test]
fn pm018_keyboard_activation_breadcrumb_intent_and_no_dropdown_stay_clean() {
    let navigation = breadcrumbs_sink();
    let (controller, sink) = dropdown_menu_handle();
    let mut breadcrumbs = Breadcrumbs::new(navigation.clone());
    breadcrumbs.set_crumbs(&[(CrumbId(1), "Past", true), (CrumbId(2), "Current", true)]);
    breadcrumbs.set_current_crumb(Some(CrumbId(2)));
    breadcrumbs.enable_dropdown(controller.clone(), sink, vec![action()]);
    let mut fixture = ActivationFixture::new(breadcrumbs);
    assert!(fixture.activate());
    assert_eq!(navigation.get(), Some(CrumbId(1)));
    assert!(!controller.borrow().is_open());
    fixture.assert_clean();

    let navigation = breadcrumbs_sink();
    let mut breadcrumbs = Breadcrumbs::new(navigation.clone());
    breadcrumbs.set_crumbs(&[(CrumbId(1), "Current", true)]);
    breadcrumbs.set_current_crumb(Some(CrumbId(1)));
    let mut fixture = ActivationFixture::new(breadcrumbs);
    assert!(fixture.activate());
    assert_eq!(navigation.get(), None);
    fixture.assert_clean();
}

#[test]
fn pm018_keyboard_activation_switch_static_fallback() {
    let mut fixture = ActivationFixture::new(Switch::new());
    fixture.assert_clean();
    assert!(fixture.activate());
    assert!(fixture.widget::<Switch>().is_on());
    assert!(!fixture.widget::<Switch>().needs_animation());
    fixture.assert_paint_only();
}

#[test]
fn pm018_keyboard_activation_disclosure_static_fallback_and_noop() {
    let mut fixture = ActivationFixture::new(DisclosureSectionHeader::new("Section", false));
    fixture.assert_clean();
    assert!(fixture.activate());
    assert!(!fixture.widget::<DisclosureSectionHeader>().expanded());
    assert!(
        !fixture
            .widget::<DisclosureSectionHeader>()
            .needs_animation()
    );
    fixture.assert_paint_only();

    fixture.clear_dirty();
    assert!(fixture.activate());
    fixture.assert_clean();
}
