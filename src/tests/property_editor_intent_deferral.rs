// SPEC-HAL-337 §4.1 stage-3 companion tests: PE Tree(ToggleExpand) deferral.
// INV-337-M1: the gated funnel enqueues identity-resolved node ids during
// event routing; PropertyEditor::update applies them before layout — never
// synchronously at dispatch time.

use crate::core::widget::{UpdateContext, Widget};
use crate::engines::hierarchical::NodeMeta;
use crate::widgets::property_editor::{PEIntent, PropertyEditor, TreeIntent};

fn parent_with_two_children() -> Vec<NodeMeta> {
    vec![
        NodeMeta {
            node_id: 1,
            parent_index: None,
            depth: 0,
            has_children: true,
            label: "Root".to_string(),
        },
        NodeMeta {
            node_id: 2,
            parent_index: Some(0),
            depth: 1,
            has_children: false,
            label: "A".to_string(),
        },
        NodeMeta {
            node_id: 3,
            parent_index: Some(0),
            depth: 1,
            has_children: false,
            label: "B".to_string(),
        },
    ]
}

fn drain(pe: &mut PropertyEditor) {
    let mut cx = UpdateContext::new(None);
    pe.update(0.0, &mut cx);
}

#[test]
fn dispatch_is_deferred_until_update() {
    let mut pe = PropertyEditor::new();
    let h = pe.hts_handle();
    h.borrow_mut().set_tree_data(parent_with_two_children());
    h.borrow_mut().set_viewport_height(200.0);

    let count_before = h.borrow_mut().visible_rows().len();
    let rev_before = h.borrow().revision();

    let accepted = pe.dispatch_intent(PEIntent::Tree(TreeIntent::ToggleExpand { row_index: 0 }));
    assert!(accepted);

    // INV-337-M1: nothing applied at dispatch time.
    assert_eq!(h.borrow_mut().visible_rows().len(), count_before);
    assert_eq!(h.borrow().revision(), rev_before);

    drain(&mut pe);

    // Applied by the hook: the root toggled, the projection changed.
    assert_ne!(h.borrow_mut().visible_rows().len(), count_before);
    assert!(h.borrow().revision() > rev_before);
}

#[test]
fn toggle_round_trip_restores_projection() {
    let mut pe = PropertyEditor::new();
    let h = pe.hts_handle();
    h.borrow_mut().set_tree_data(parent_with_two_children());
    h.borrow_mut().set_viewport_height(200.0);

    let count_before = h.borrow_mut().visible_rows().len();

    let _ = pe.dispatch_intent(PEIntent::Tree(TreeIntent::ToggleExpand { row_index: 0 }));
    drain(&mut pe);
    let _ = pe.dispatch_intent(PEIntent::Tree(TreeIntent::ToggleExpand { row_index: 0 }));
    drain(&mut pe);

    assert_eq!(h.borrow_mut().visible_rows().len(), count_before);
}

#[test]
fn empty_queue_update_is_noop() {
    let mut pe = PropertyEditor::new();
    let h = pe.hts_handle();
    h.borrow_mut().set_tree_data(parent_with_two_children());
    h.borrow_mut().set_viewport_height(200.0);

    let rev_before = h.borrow().revision();
    drain(&mut pe);
    assert_eq!(h.borrow().revision(), rev_before);
}
