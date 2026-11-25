// SPEC-HAL-337 §2/§8 stage-2 tests: TreeView ownership surface.
// INV-337-O1: one substrate instance per topology — handle identity holds
// through both constructors, and mutations are visible through every path.

use std::cell::RefCell;
use std::rc::Rc;

use crate::engines::hierarchical::{
    HierarchicalTabularSubstrate, HtsOwnership, NodeMeta, SharedHts,
};
use crate::widgets::tree_view::TreeView;

/// Three root nodes, no children.
fn three_roots() -> Vec<NodeMeta> {
    vec![
        NodeMeta {
            node_id: 10,
            parent_index: None,
            depth: 0,
            has_children: false,
            label: "Root A".to_string(),
        },
        NodeMeta {
            node_id: 20,
            parent_index: None,
            depth: 0,
            has_children: false,
            label: "Root B".to_string(),
        },
        NodeMeta {
            node_id: 30,
            parent_index: None,
            depth: 0,
            has_children: false,
            label: "Root C".to_string(),
        },
    ]
}

#[test]
fn internal_ownership_mints_and_exposes_handle() {
    let view = TreeView::new();
    assert_eq!(view.ownership(), HtsOwnership::Internal);

    // The exposed handle is the widget's instance (INV-337-O1).
    let h = view.hts_handle();
    assert!(Rc::ptr_eq(&h, &view.hts_handle()));

    // Driving the internal substrate through the exposed handle is
    // observable through the widget.
    h.borrow_mut().set_tree_data(three_roots());
    h.borrow_mut().set_viewport_height(200.0);
    assert_eq!(view.hts_mut().visible_rows().len(), 3);
}

#[test]
fn external_ownership_shares_injected_instance() {
    let hts: SharedHts = Rc::new(RefCell::new(HierarchicalTabularSubstrate::new()));
    let view = TreeView::with_hts(Rc::clone(&hts));
    assert_eq!(view.ownership(), HtsOwnership::External);
    assert!(Rc::ptr_eq(&view.hts_handle(), &hts));
}

#[test]
fn mutate_via_handle_observe_via_widget() {
    let hts: SharedHts = Rc::new(RefCell::new(HierarchicalTabularSubstrate::new()));
    let view = TreeView::with_hts(Rc::clone(&hts));

    hts.borrow_mut().set_tree_data(three_roots());
    hts.borrow_mut().set_viewport_height(200.0);

    let rows = view.hts_mut().visible_rows();
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].node_id, 10);
    assert_eq!(rows[1].node_id, 20);
    assert_eq!(rows[2].node_id, 30);

    // Revision agrees through both read paths — same instance, not a copy.
    assert_eq!(view.revision(), hts.borrow().revision());
}

#[test]
fn widget_mutation_visible_via_handle() {
    // Reverse direction: widget API routes through the CMS-held handle;
    // the injected handle observes it (INV-337-O1 — with_handle threads
    // ONE instance into the CMS and the widget).
    let hts: SharedHts = Rc::new(RefCell::new(HierarchicalTabularSubstrate::new()));
    let mut view = TreeView::with_hts(Rc::clone(&hts));

    view.set_row_height(32.0);
    assert_eq!(hts.borrow().row_height(), 32.0);
}
