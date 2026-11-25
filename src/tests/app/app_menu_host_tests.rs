    use super::*;
    use crate::model::menu_item::MenuItem;
    use crate::model::menu_model::MenuModel;
    use crate::runtime::actions::{Action, ActionId, ActionScope};
    use crate::widgets::menu_bar::{InWindowMenuHost, MenuBar, MenuBarMailbox};

    #[derive(Default)]
    struct RoleState {
        initialized: usize,
        quit: bool,
        installed: usize,
        ordinary_drains: usize,
    }
    struct RoleHost(Rc<RefCell<RoleState>>);
    impl crate::platform::menu_host::MenuHost for RoleHost {
        fn initialize_application_roles(&mut self, wake: Option<UiWakeHandle>) {
            assert!(wake.is_none(), "manual owner fixture does not forge a native waker");
            self.0.borrow_mut().initialized += 1;
        }
        fn take_quit_request(&mut self) -> bool { std::mem::take(&mut self.0.borrow_mut().quit) }
        fn install(&mut self, _: &MenuModel, _: &ActionRegistry) { self.0.borrow_mut().installed += 1; }
        fn sync_item(&mut self, _: &MenuModel, _: crate::model::menu_item::MenuItemId) {}
        fn drain_activations(&mut self) -> Vec<crate::model::menu_item::MenuItemId> {
            self.0.borrow_mut().ordinary_drains += 1;
            vec![1]
        }
        fn occupies_in_window_bar(&self) -> bool { false }
    }

    #[test]
    fn standard_roles_initialize_without_model_and_terminal_drain_does_not_dispatch() {
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(ScreenHost::new()), None, 0);
        let mut app = AppRoot::new(AppContext::new(Theme::dark()), root);
        let roles = Rc::new(RefCell::new(RoleState::default()));
        app.set_menu_host(Box::new(RoleHost(roles.clone())), None);
        assert_eq!(roles.borrow().initialized,1);
        assert!(app.context.menu_model.is_none());
        assert!(!app.context.exit_requested());
        roles.borrow_mut().quit = true;
        assert!(app.take_standard_quit_request());
        assert!(!app.take_standard_quit_request());
        assert!(app.context.exit_requested());
        assert_eq!(roles.borrow().ordinary_drains,0);
        assert_eq!(roles.borrow().installed,0);
    }

    #[test]
    fn standard_quit_wins_normal_sync_without_product_install_or_action() {
        let mut r=rig();
        let roles=Rc::new(RefCell::new(RoleState::default()));
        r.app_root.set_menu_host(Box::new(RoleHost(roles.clone())),None);
        r.app_root.context.registry.add(Action {id:ActionId(2000),name:"Ordinary item",description:"",category:"Test",
            shortcut:None,scope:ActionScope::Global,handler:invalidate_handler});
        let mut model=MenuModel::new();model.add_title("File",vec![row(1,"Ordinary item")]);
        model.bind(1,ActionId(2000),&r.app_root.context.registry);
        r.app_root.context.set_menu_model(model);
        roles.borrow_mut().quit=true;
        r.app_root.sync_menu_host(&mut r.tree);
        assert!(r.app_root.context.exit_requested());
        assert_eq!(roles.borrow().installed,0);
        assert_eq!(roles.borrow().ordinary_drains,0);
        assert!(!r.app_root.context.take_invalidate_requested());
    }

    #[test]
    fn external_in_window_host_keeps_default_standard_role_behavior() {
        let mut r=rig();
        assert!(!r.app_root.take_standard_quit_request());
        assert!(!r.app_root.context.exit_requested());
    }

    fn invalidate_handler(cx: &mut AppContext) {
        cx.request_invalidate();
    }

    fn row(id: u32, label: &str) -> MenuItem {
        MenuItem::ActionRow {
            id,
            label: label.to_string(),
            icon: None,
            shortcut: None,
            disabled: false,
            submenu: None,
        }
    }

    struct Rig {
        tree: WidgetTree,
        app_root: AppRoot,
        bar_node: NodeId,
        mailbox: Rc<RefCell<MenuBarMailbox>>,
    }

    fn rig() -> Rig {
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(ScreenHost::new()), None, 0);
        let mut app_root = AppRoot::new(AppContext::new(Theme::dark()), root);

        // The App::run wiring shape: mailbox, mounted bar, in-window host.
        let mailbox = Rc::new(RefCell::new(MenuBarMailbox::default()));
        let bar = MenuBar::new(Rc::clone(&mailbox));
        let bar_node = tree.add_child(root, Box::new(bar), None, 100);
        app_root.set_menu_host(
            Box::new(InWindowMenuHost::new(Rc::clone(&mailbox))),
            Some(bar_node),
        );

        Rig {
            tree,
            app_root,
            bar_node,
            mailbox,
        }
    }

    #[test]
    fn no_host_sync_is_no_op() {
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(ScreenHost::new()), None, 0);
        let mut app_root = AppRoot::new(AppContext::new(Theme::dark()), root);
        app_root.sync_menu_host(&mut tree);
    }

    #[test]
    fn install_applies_once_and_marks_bar_dirty() {
        let mut r = rig();
        let mut model = MenuModel::new();
        model.add_title("File", vec![row(1, "Do")]);
        r.app_root.context.set_menu_model(model);

        r.tree.node_mut(r.bar_node).unwrap().dirty_layout = false;
        r.app_root.sync_menu_host(&mut r.tree);
        assert!(
            r.tree.node(r.bar_node).unwrap().dirty_layout,
            "install must mark the bar layout-dirty"
        );

        // Nothing changed: second frame applies nothing, no dirty.
        r.tree.node_mut(r.bar_node).unwrap().dirty_layout = false;
        r.app_root.sync_menu_host(&mut r.tree);
        assert!(!r.tree.node(r.bar_node).unwrap().dirty_layout);
    }

    #[test]
    fn model_mutation_syncs_and_marks_bar_dirty() {
        let mut r = rig();
        let mut model = MenuModel::new();
        model.add_title("File", vec![row(1, "Do")]);
        r.app_root.context.set_menu_model(model);
        r.app_root.sync_menu_host(&mut r.tree); // install frame

        r.app_root
            .context
            .menu_model_mut()
            .unwrap()
            .set_enabled(1, false);

        r.tree.node_mut(r.bar_node).unwrap().dirty_layout = false;
        r.app_root.sync_menu_host(&mut r.tree);
        assert!(
            r.tree.node(r.bar_node).unwrap().dirty_layout,
            "item sync must reach the bar and mark it dirty"
        );
    }

    #[test]
    fn activation_dispatches_bound_app_action() {
        let mut r = rig();
        r.app_root.context.registry.add(Action {
            id: ActionId(2000),
            name: "Test Menu Action",
            description: "",
            category: "Test",
            shortcut: None,
            scope: ActionScope::Global,
            handler: invalidate_handler,
        });
        let mut model = MenuModel::new();
        model.add_title("File", vec![row(1, "Do")]);
        model.bind(1, ActionId(2000), &r.app_root.context.registry);
        r.app_root.context.set_menu_model(model);
        r.app_root.sync_menu_host(&mut r.tree); // install frame

        // Activation arrives from the bar (test injector).
        r.mailbox.borrow_mut().push_activation_for_test(1);
        r.app_root.sync_menu_host(&mut r.tree);

        assert!(
            r.app_root.context.take_invalidate_requested(),
            "bound app action handler must execute on activation"
        );
    }
