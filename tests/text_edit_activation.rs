use halcyon::prelude::{TextEdit, Widget};

#[test]
fn text_edit_preserves_initial_text() {
    let edit = TextEdit::with_text("alpha\nbeta");

    assert_eq!(edit.text(), "alpha\nbeta");
}

#[test]
fn text_edit_exposes_active_editor_contract() {
    let edit = TextEdit::new();

    assert!(edit.focusable());
    assert!(edit.capture_eligible());
    assert!(edit.wants_scroll());
    assert_eq!(edit.debug_type_name(), "TextEdit");
}
