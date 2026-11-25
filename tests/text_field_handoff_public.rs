//! Ordinary public consumer compilation and stage-only behavior.
//! No test-harness APIs, native input, engine cut or physical output is claimed here.

use halcyon::core::app::{AppContext, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::theme::Theme;
use halcyon::core::widget::Widget;
use halcyon::core::widget_tree::{NodeId, WidgetTree};
use halcyon::widgets::flex::Flex;
use halcyon::widgets::text_field::{
    TextField, TextFieldEditHandoff, TextFieldHandoffError, TextFieldHandoffStatus,
};

#[derive(Default)]
struct HandoffScreen {
    saved: Option<TextFieldEditHandoff>,
    node: Option<NodeId>,
    refusal: Option<TextFieldHandoffError>,
}

impl Screen for HandoffScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        let root = cx.mount_child(Box::new(Flex::column()));
        let mut target = TextField::new();
        if let Some(saved) = &self.saved {
            target.try_stage_edit_handoff(saved).unwrap();
        }
        self.node = Some(cx.tree.add_child(root, Box::new(target), None, 0));
        cx.tree.node_mut(root).unwrap().widget_mut()
    }

    fn flush_pending_actions(&mut self, cx: &mut ScreenContext<'_>) {
        let Some(field) = cx.tree.as_deref().and_then(|tree| {
            self.node.and_then(|node| {
                tree.node(node)?
                    .widget()
                    .as_any()
                    .downcast_ref::<TextField>()
            })
        }) else {
            return;
        };
        // Keep the original value until the mounted owner has actually applied it.
        match field.edit_handoff_status() {
            TextFieldHandoffStatus::Pending => {}
            TextFieldHandoffStatus::Rejected(error) => self.refusal = Some(error),
            TextFieldHandoffStatus::NotRequested | TextFieldHandoffStatus::Applied => {
                match field.capture_edit_handoff() {
                    Ok(saved) => self.saved = Some(saved),
                    Err(error) => self.refusal = Some(error),
                }
            }
        }
    }
}

#[test]
fn edit_handoff_public_consumer_stages_without_publishing() {
    let mut original = TextField::new().with_text("Вопрос é🙂");
    original.select_all();
    let mut source_tree = WidgetTree::new();
    let source = source_tree.create_root(Box::new(original), None, 0);
    let saved = source_tree
        .node(source)
        .unwrap()
        .widget()
        .as_any()
        .downcast_ref::<TextField>()
        .unwrap()
        .capture_edit_handoff()
        .unwrap();
    assert_eq!(saved.text(), "Вопрос é🙂");
    assert_eq!(saved.selection_anchor_byte(), 0);
    assert_eq!(saved.selection_head_byte(), saved.text().len());
    assert_eq!(format!("{saved:?}"), "TextFieldEditHandoff { .. }");
    let mut screen = HandoffScreen {
        saved: Some(saved.clone()),
        ..Default::default()
    };
    let mut target_tree = WidgetTree::new();
    let host = target_tree.create_root(Box::new(Flex::column()), None, 0);
    let mut app = AppContext::new(Theme::default());
    {
        let mut cx = ScreenFrameContext {
            app: &mut app,
            tree: &mut target_tree,
            parent_node: host,
        };
        let _ = screen.build(&mut cx);
    }
    let field = target_tree
        .node(screen.node.unwrap())
        .unwrap()
        .widget()
        .as_any()
        .downcast_ref::<TextField>()
        .unwrap();
    assert_eq!(field.edit_handoff_status(), TextFieldHandoffStatus::Pending);
    assert!(field.text().is_empty());
    assert_eq!(screen.saved.as_ref().unwrap().text(), saved.text());
}
