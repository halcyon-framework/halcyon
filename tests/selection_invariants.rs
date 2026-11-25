// SPEC-HAL-107-V
// Any failing test indicates a violation of SPEC-HAL-107.
// Demo behavior is non-authoritative.

use halcyon::core::editing::{Selection, TextCursor};

#[test]
fn collapsed_selection_anchor_equals_head() {
    let selection = Selection::at(3);

    assert_eq!(selection.anchor, 3);
    assert_eq!(selection.head, 3);
    assert!(selection.is_empty());
}

#[test]
fn selection_order_is_irrelevant() {
    let forward = Selection { anchor: 2, head: 5 };
    let backward = Selection { anchor: 5, head: 2 };

    assert_eq!(forward.start(), backward.start());
    assert_eq!(forward.end(), backward.end());
    assert_eq!(forward.start()..forward.end(), 2..5);
}

#[test]
fn extending_selection_moves_head_only() {
    let mut selection = Selection::at(2);

    selection.extend_to(6);

    assert_eq!(selection.anchor, 2);
    assert_eq!(selection.head, 6);
    assert_eq!(selection.start()..selection.end(), 2..6);
}

#[test]
fn collapsing_selection_sets_anchor_and_head() {
    let mut selection = Selection { anchor: 1, head: 5 };
    let cursor = TextCursor::at(4);

    selection.clear(cursor.pos);

    assert_eq!(selection.anchor, 4);
    assert_eq!(selection.head, 4);
    assert!(selection.is_empty());
}

#[test]
fn selection_never_exits_document_bounds() {
    let text = "abcde";
    let mut selection = Selection::at(1);

    selection.extend_to(text.len());

    assert!(selection.start() <= text.len());
    assert!(selection.end() <= text.len());
    assert_eq!(selection.start()..selection.end(), 1..5);
}
