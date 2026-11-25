// SPEC-HAL-337 §4.1 stage-3 tests: TreeView topology-intent funnel.
// INV-337-M1: topology intents enqueued during event routing are applied
// by the per-frame hook (TreeView::update) before layout — never
// synchronously at enqueue time.

use crate::core::widget::{UpdateContext, Widget};
use crate::engines::hierarchical::NodeMeta;
use crate::widgets::tree_view::{TreeTopologyIntent, TreeView};

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

fn drain(view: &mut TreeView) {
    let mut cx = UpdateContext::new(None);
    view.update(0.0, &mut cx);
}

#[test]
fn enqueue_is_deferred_until_update() {
    let mut view = TreeView::new();
    let h = view.hts_handle();
    h.borrow_mut().set_tree_data(parent_with_two_children());
    h.borrow_mut().set_viewport_height(200.0);

    let count_before = h.borrow_mut().visible_rows().len();
    let rev_before = h.borrow().revision();

    view.enqueue_topology_intent(TreeTopologyIntent::Toggle { node_id: 1 });

    // INV-337-M1: nothing applied at enqueue time.
    assert_eq!(h.borrow_mut().visible_rows().len(), count_before);
    assert_eq!(h.borrow().revision(), rev_before);

    drain(&mut view);

    // Applied by the hook: the root toggled, the projection changed.
    assert_ne!(h.borrow_mut().visible_rows().len(), count_before);
    assert!(h.borrow().revision() > rev_before);
}

#[test]
fn toggle_round_trip_restores_projection() {
    let mut view = TreeView::new();
    let h = view.hts_handle();
    h.borrow_mut().set_tree_data(parent_with_two_children());
    h.borrow_mut().set_viewport_height(200.0);

    let count_before = h.borrow_mut().visible_rows().len();

    view.enqueue_topology_intent(TreeTopologyIntent::Toggle { node_id: 1 });
    drain(&mut view);
    view.enqueue_topology_intent(TreeTopologyIntent::Toggle { node_id: 1 });
    drain(&mut view);

    assert_eq!(h.borrow_mut().visible_rows().len(), count_before);
}

#[test]
fn empty_queue_update_is_noop() {
    let mut view = TreeView::new();
    let h = view.hts_handle();
    h.borrow_mut().set_tree_data(parent_with_two_children());
    h.borrow_mut().set_viewport_height(200.0);

    let rev_before = h.borrow().revision();
    drain(&mut view);
    assert_eq!(h.borrow().revision(), rev_before);
}
