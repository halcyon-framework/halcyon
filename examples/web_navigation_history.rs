//! Ordinary App route projection with a draft kept in application memory.

#[cfg(not(target_arch = "wasm32"))]
pub fn main() {}

#[cfg(target_arch = "wasm32")]
mod browser {
    use halcyon::core::app::{App, AppConfig, Screen, ScreenFrameContext, WebNavigation};
    use halcyon::core::appearance::ButtonMaterialPolicy;
    use halcyon::core::layout::{EdgeInsets, FlexParams};
    use halcyon::core::widget::Widget;
    use halcyon::model::navigation_host::{
        GuardDecision, NavigationGuard, NavigationIntent, NavigationNodeId, NavigationState,
    };
    use halcyon::prelude::{Button, Flex, Label, TextField};
    use std::any::Any;
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    const HOME: NavigationNodeId = NavigationNodeId(1);
    const CONFIRM: NavigationNodeId = NavigationNodeId(2);

    struct BackGuard(Rc<Cell<bool>>);

    impl NavigationGuard for BackGuard {
        fn evaluate(
            &self,
            requested: &NavigationIntent,
            _: &NavigationIntent,
            _: &NavigationState,
            _: &NavigationState,
            _: Option<&dyn Any>,
        ) -> GuardDecision {
            if self.0.get() && *requested == NavigationIntent::HistoryBack {
                GuardDecision::Deny
            } else {
                GuardDecision::Allow
            }
        }
    }

    struct RoutesScreen {
        placeholder: Label,
        rebuild: bool,
        draft: Rc<RefCell<String>>,
        deny_back: Rc<Cell<bool>>,
    }

    impl Screen for RoutesScreen {
        fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
            let projection = cx
                .app
                .web_navigation_projection()
                .expect("declared public routes");
            let home = cx.app.web_navigation_path(&projection.current) == Some("/");
            let route = if home { "Route Home" } else { "Route Confirm" };
            let back = if projection.can_history_back {
                "Yes"
            } else {
                "No"
            };
            let forward = if projection.can_history_forward {
                "Yes"
            } else {
                "No"
            };
            let status = format!("Back {back}   Forward {forward}");
            let intents = cx.app.web_navigation_intents().expect("declared root");
            let column = cx.mount_child(Box::new(
                Flex::column()
                    .with_padding(EdgeInsets::all(16.0))
                    .with_spacing(6.0),
            ));
            let route_node = cx
                .tree
                .add_child(column, Box::new(Label::new(route)), None, 0);
            cx.tree.set_flex(route_node, FlexParams::fixed(30.0));
            let history = cx
                .tree
                .add_child(column, Box::new(Label::new(status)), None, 0);
            cx.tree.set_flex(history, FlexParams::fixed(26.0));
            let body: Box<dyn Widget> = if home {
                let draft = self.draft.clone();
                let mut field = TextField::new()
                    .with_placeholder("Draft")
                    .with_text(self.draft.borrow().as_str());
                field.set_on_change(move |text| *draft.borrow_mut() = text.to_owned());
                Box::new(field)
            } else {
                Box::new(Label::new(format!(
                    "Draft length {}",
                    self.draft.borrow().len()
                )))
            };
            let body = cx.tree.add_child(column, body, None, 0);
            cx.tree.set_flex(body, FlexParams::fixed(40.0));
            for (title, intent) in [
                ("Continue", NavigationIntent::NavigateTo(CONFIRM.into())),
                ("Logo Home", NavigationIntent::NavigateTo(HOME.into())),
                ("Back", NavigationIntent::HistoryBack),
                ("Forward", NavigationIntent::HistoryForward),
                ("Up", NavigationIntent::NavigateUp),
            ] {
                let intents = intents.clone();
                let button = Button::with_text(title).on_click(move || {
                    if let Err(error) = intents.request(intent.clone()) {
                        tracing::warn!(%error, "route request not queued");
                    }
                });
                let node = cx.tree.add_child(column, Box::new(button), None, 0);
                cx.tree.set_flex(node, FlexParams::fixed(34.0));
            }
            let deny_back = self.deny_back.clone();
            let guard = Button::with_text("Toggle Back Guard")
                .on_click(move || deny_back.set(!deny_back.get()));
            let node = cx.tree.add_child(column, Box::new(guard), None, 0);
            cx.tree.set_flex(node, FlexParams::fixed(34.0));
            &mut self.placeholder
        }

        fn mark_rebuild(&mut self) {
            self.rebuild = true;
        }

        fn take_rebuild_flag(&mut self) -> bool {
            std::mem::take(&mut self.rebuild)
        }
    }

    #[wasm_bindgen::prelude::wasm_bindgen(start)]
    pub fn start() {
        std::panic::set_hook(Box::new(|message| {
            web_sys::console::error_1(&message.to_string().into());
        }));
        let deny_back = Rc::new(Cell::new(false));
        let routes = WebNavigation::new(HOME, [(HOME, "/"), (CONFIRM, "/confirm")])
            .expect("static public route mapping")
            .with_guard(BackGuard(deny_back.clone()));
        let mut config = AppConfig::new("Halcyon Web Navigation")
            .with_size(390.0, 640.0)
            .with_web_navigation(routes);
        config.theme.button_material_policy = ButtonMaterialPolicy::SdfV3;
        App::new(config).run(RoutesScreen {
            placeholder: Label::new(""),
            rebuild: true,
            draft: Rc::new(RefCell::new(String::new())),
            deny_back,
        });
    }
}
