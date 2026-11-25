use crate::core::app::RetainedSceneTestHarness;
use crate::core::event::{
    FusedInputEvent, InteractionDomains, KeyCode, KeyboardAuthorityMode, Modifiers,
    PhysicalKeyCode, WindowId,
};
use crate::core::event_router::EventRouter;
use crate::core::icon::IconRegistry;
use crate::core::layout::Constraints;
use crate::core::math::Size;
use crate::core::theme::Theme;
use crate::core::widget_tree::{NodeId, WidgetTree};
use crate::model::default_selection_resolver::DefaultSelectionResolver;
use crate::model::search_projection_harness::SearchProjectionHarness;
use crate::model::selection_resolver::{CommittedValue, OptionId, ResolverHandle};
use crate::platform::font_manager_text_engine::FontManagerTextEngine;
use crate::runtime::actions::ActionId;
use crate::runtime::binding_registry::BindingRegistry;
use crate::widgets::{ComboBox, flex::Flex, text_field::TextField};
use web_time::Instant;

struct Harness {
    tree: WidgetTree,
    root: NodeId,
    combo_node: NodeId,
    text_engine: FontManagerTextEngine,
    frame: RetainedSceneTestHarness,
    router: EventRouter,
    domains: InteractionDomains,
    bindings: BindingRegistry,
}

impl Harness {
    fn new(labels: &[&str], search: bool, projection: bool) -> Self {
        let options: Vec<_> = labels
            .iter()
            .enumerate()
            .map(|(index, label)| (OptionId::from_raw(index as u64), *label))
            .collect();
        let mut combo = ComboBox::new(ResolverHandle::new(DefaultSelectionResolver::strict()));
        combo.set_options(&options);
        combo.set_type_to_search(search);
        if projection {
            combo.set_search_harness(SearchProjectionHarness::new(&options));
        }

        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(Flex::column()), None, 0);
        let combo_node = tree.add_child(root, Box::new(combo), None, 0);
        let owner = tree.node(combo_node).unwrap().id;
        let children = tree.children(combo_node);
        assert_eq!(children.len(), 1, "ComboBox must mount its TextField");
        assert!(
            tree.node(children[0]).unwrap().widget.as_any().is::<TextField>(),
            "the mounted child must be the actual editor"
        );

        let mut text_engine = FontManagerTextEngine::new();
        let mut frame = RetainedSceneTestHarness::new();
        let mut domains = InteractionDomains::new();
        frame.frame_with_appearance(
            &mut tree,
            root,
            Constraints::tight(Size::new(240.0, 180.0)),
            &Theme::light(),
            &mut text_engine,
            &IconRegistry::new(&[]),
            &domains,
            WindowId::PRIMARY,
        );

        let mut router = EventRouter::new();
        let domain = domains.primary_state_mut();
        router.request_focus_for_widget(&mut tree, owner, domain);
        assert!(router.handle_engine_action(
            &mut tree,
            ActionId::ENTER_WIDGET,
            Some(root),
            domain,
            &mut text_engine,
            Some(frame.animation_context()),
        ));
        assert_eq!(domain.focus_manager.focused(), Some(owner));
        assert_eq!(domain.keyboard_mode, KeyboardAuthorityMode::Capture);

        Self {
            tree,
            root,
            combo_node,
            text_engine,
            frame,
            router,
            domains,
            bindings: BindingRegistry::new(),
        }
    }

    fn combo(&self) -> &ComboBox {
        self.tree
            .node(self.combo_node)
            .unwrap()
            .widget
            .as_any()
            .downcast_ref::<ComboBox>()
            .unwrap()
    }

    fn combo_mut(&mut self) -> &mut ComboBox {
        self.tree
            .node_mut(self.combo_node)
            .unwrap()
            .widget_mut()
            .as_any_mut()
            .downcast_mut::<ComboBox>()
            .unwrap()
    }

    fn dispatch(&mut self, event: FusedInputEvent) {
        self.router.route_input_event(
            &mut self.tree,
            event,
            Some(self.root),
            &mut self.text_engine,
            Some(self.frame.animation_context()),
            self.domains.primary_state_mut(),
            &mut self.bindings,
        );
    }

    fn text(&mut self, text: &str) {
        for character in text.chars() {
            self.dispatch(FusedInputEvent::TextInput {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                text: character.to_string(),
                modifiers: Modifiers::default(),
            });
        }
    }

    fn key(&mut self, key_code: KeyCode, physical_key: PhysicalKeyCode) {
        self.dispatch(FusedInputEvent::KeyDown {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            key_code,
            physical_key,
            reservation: None,
            modifiers: Modifiers::default(),
            repeat: false,
        });
    }
}

#[test]
fn combo_box_text_input_updates_search_and_completion_without_leaving_typing() {
    for projection in [false, true] {
        let mut harness = Harness::new(&["other", "jki-node"], true, projection);

        harness.text("j");
        assert_eq!(harness.combo().input_text(), "j");
        assert_eq!(harness.combo().test_highlighted_label(), Some("jki-node"));
        assert_eq!(harness.combo().test_ghost_completion(), Some("ki-node"));

        harness.text("ki");
        assert_eq!(harness.combo().input_text(), "jki");
        assert_eq!(harness.combo().test_ghost_completion(), Some("-node"));
        assert_eq!(harness.combo_mut().take_committed(), None);

        harness.key(KeyCode::Enter, PhysicalKeyCode::Enter);
        assert_eq!(
            harness.combo_mut().take_committed(),
            None,
            "a search hint must not turn strict InputConfirm into Selection"
        );
        assert_eq!(harness.combo().input_text(), "jki");
    }
}

#[test]
fn combo_box_option_list_jki_navigates_and_commits_without_typing() {
    let mut harness = Harness::new(&["Alpha", "Beta"], false, false);
    harness.key(KeyCode::Down, PhysicalKeyCode::ArrowDown);
    assert_eq!(harness.combo().test_highlighted_label(), Some("Alpha"));

    harness.text("j");
    assert_eq!(harness.combo().test_highlighted_label(), Some("Beta"));
    harness.text("k");
    assert_eq!(harness.combo().test_highlighted_label(), Some("Alpha"));
    assert_eq!(harness.combo().input_text(), "");

    harness.text("i");
    assert_eq!(
        harness.combo_mut().take_committed(),
        Some(CommittedValue::ExistingOption(OptionId::from_raw(0)))
    );
    assert_eq!(harness.combo().input_text(), "Alpha");
}

#[test]
fn combo_box_typing_after_list_navigation_restores_text_input_zone() {
    let mut harness = Harness::new(&["other", "local-jki"], true, true);
    harness.key(KeyCode::Down, PhysicalKeyCode::ArrowDown);
    assert_eq!(harness.combo().test_highlighted_label(), Some("other"));

    harness.text("local-jki");
    assert_eq!(harness.combo().input_text(), "local-jki");
    assert_eq!(harness.combo().test_highlighted_label(), Some("local-jki"));
    assert_eq!(harness.combo_mut().take_committed(), None);
}

#[test]
fn combo_box_loading_keeps_text_editing_without_local_search() {
    let mut harness = Harness::new(&["jki-node"], true, true);
    harness.combo_mut().set_loading(true);

    harness.text("jki");
    assert_eq!(harness.combo().input_text(), "jki");
    assert_eq!(harness.combo().test_highlighted_label(), None);
    assert_eq!(harness.combo().test_ghost_completion(), None);
    assert_eq!(harness.combo_mut().take_committed(), None);
}
