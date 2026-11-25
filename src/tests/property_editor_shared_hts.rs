// SPEC-HAL-337 §2/§8 stage-4 tests: PropertyEditor ownership surface.
// INV-337-O1: one substrate instance per topology — handle identity holds
// through both constructors; mutations are visible through every path;
// injected substrates are never reconfigured by construction.

use std::cell::RefCell;
use std::rc::Rc;

use crate::engines::hierarchical::{HierarchicalTabularSubstrate, HtsOwnership, SharedHts};
use crate::widgets::property_editor::PropertyEditor;

#[test]
fn internal_ownership_mints_and_exposes_handle() {
    let pe = PropertyEditor::new();
    assert_eq!(pe.ownership(), HtsOwnership::Internal);

    // The exposed handle is the widget's instance (INV-337-O1).
    let h = pe.hts_handle();
    assert!(Rc::ptr_eq(&h, &pe.hts_handle()));

    // new() configured the minted substrate (DEFAULT_ROW_HEIGHT = 24.0).
    assert_eq!(h.borrow().row_height(), 24.0);
}

#[test]
fn external_ownership_shares_injected_instance() {
    let hts: SharedHts = Rc::new(RefCell::new(HierarchicalTabularSubstrate::new()));
    let pe = PropertyEditor::with_hts(Rc::clone(&hts));
    assert_eq!(pe.ownership(), HtsOwnership::External);
    assert!(Rc::ptr_eq(&pe.hts_handle(), &hts));
}

#[test]
fn widget_mutation_visible_via_handle() {
    // Widget builder API routes through the CMS-held handle; the injected
    // handle observes it (INV-337-O1 — with_handle threads ONE instance).
    let hts: SharedHts = Rc::new(RefCell::new(HierarchicalTabularSubstrate::new()));
    let mut pe = PropertyEditor::with_hts(Rc::clone(&hts));

    let rev_before = hts.borrow().revision();
    pe.add_name_column("Name");
    assert!(hts.borrow().revision() > rev_before);
}

#[test]
fn injected_substrate_not_reconfigured() {
    // with_hts must not stomp the application's substrate configuration:
    // set_row_height(DEFAULT_ROW_HEIGHT) belongs to new() only.
    let hts: SharedHts = Rc::new(RefCell::new(HierarchicalTabularSubstrate::new()));
    hts.borrow_mut().set_row_height(32.0);

    let _pe = PropertyEditor::with_hts(Rc::clone(&hts));
    assert_eq!(hts.borrow().row_height(), 32.0);
}
