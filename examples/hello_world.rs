//! A small example of a native screen and a button that updates a counter.
//! Run with `cargo run --example hello_world`.

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::appearance::ButtonMaterialPolicy;
use halcyon::core::event::FusedInputEvent;
use halcyon::core::layout::{EdgeInsets, FlexParams};
use halcyon::core::widget::Widget;
use halcyon::core::widget_tree::{NodeId, WidgetTree};
use halcyon::prelude::{Button, Flex, Label};
use std::cell::Cell;
use std::rc::Rc;

fn main() {
    let mut config = AppConfig::new("Halcyon - Hello World").with_size(440.0, 260.0);
    config.theme.button_material_policy = ButtonMaterialPolicy::SdfV3;
    App::new(config).run(HelloWorld {
        placeholder: Label::new(""),
        rebuild: true,
        clicks: Rc::new(Cell::new(0)),
        status: None,
        displayed_clicks: 0,
    });
}

struct HelloWorld {
    placeholder: Label,
    rebuild: bool,
    clicks: Rc<Cell<u64>>,
    status: Option<NodeId>,
    displayed_clicks: u64,
}

impl Screen for HelloWorld {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        let column = cx.mount_child(Box::new(
            Flex::column()
                .with_padding(EdgeInsets::all(20.0))
                .with_spacing(12.0),
        ));
        let greeting = cx
            .tree
            .add_child(column, Box::new(Label::new("Hello, world!")), None, 0);
        cx.tree.set_flex(greeting, FlexParams::fixed(24.0));

        let clicks = Rc::clone(&self.clicks);
        let button = Button::with_text("Click me").on_click(move || {
            clicks.set(clicks.get().checked_add(1).expect("click count overflow"));
        });
        let button = cx.tree.add_child(column, Box::new(button), None, 0);
        cx.tree.set_flex(button, FlexParams::fixed(42.0));

        self.displayed_clicks = self.clicks.get();
        let status = cx.tree.add_child(
            column,
            Box::new(Label::new(format!("Clicks: {}", self.displayed_clicks))),
            None,
            0,
        );
        cx.tree.set_flex(status, FlexParams::fixed(23.0));
        self.status = Some(status);
        &mut self.placeholder
    }

    fn handle_input(&mut self, _event: &FusedInputEvent, _cx: &mut ScreenContext<'_>) -> bool {
        false
    }

    fn update(&mut self, tree: &mut WidgetTree) {
        let clicks = self.clicks.get();
        if clicks == self.displayed_clicks {
            return;
        }
        let Some(status) = self.status else {
            return;
        };
        let Some(label) = tree
            .node_mut(status)
            .and_then(|node| node.widget_mut().as_any_mut().downcast_mut::<Label>())
        else {
            return;
        };
        label.set_text(format!("Clicks: {clicks}"));
        tree.mark_layout_dirty(status);
        self.displayed_clicks = clicks;
    }

    fn mark_rebuild(&mut self) {
        self.rebuild = true;
    }

    fn take_rebuild_flag(&mut self) -> bool {
        std::mem::take(&mut self.rebuild)
    }
}
