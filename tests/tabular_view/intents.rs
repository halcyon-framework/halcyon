// =============================================================================
// TabularSubstrate Intents Integration Tests
// =============================================================================
//
// Tests for SPEC-HAL-169, 171, 174, 175: Intent directives.
// All tests use only public APIs.
// =============================================================================

use halcyon::widgets::tabular_view::{
    ColumnId, EditIntent, ResizeIntent, RowId, SortDirection, SortIntent,
};

#[test]
fn test_sort_direction_toggle() {
    assert_eq!(SortDirection::Ascending.toggle(), SortDirection::Descending);
    assert_eq!(SortDirection::Descending.toggle(), SortDirection::Ascending);
}

#[test]
fn test_resize_intent_column_id_extraction() {
    let col_id = ColumnId::new();

    let begin = ResizeIntent::Begin {
        column_id: col_id,
        initial_width: 100.0,
    };
    assert_eq!(begin.column_id(), col_id);

    let update = ResizeIntent::Update {
        column_id: col_id,
        proposed_width: 120.0,
    };
    assert_eq!(update.column_id(), col_id);

    let commit = ResizeIntent::Commit {
        column_id: col_id,
        proposed_width: 120.0,
    };
    assert_eq!(commit.column_id(), col_id);

    let cancel = ResizeIntent::Cancel { column_id: col_id };
    assert_eq!(cancel.column_id(), col_id);
}

#[test]
fn test_sort_intent_column_id_extraction() {
    let col_id = ColumnId::new();

    let sort = SortIntent::RequestSort {
        column_id: col_id,
        direction: Some(SortDirection::Ascending),
    };
    assert_eq!(sort.column_id(), col_id);

    let clear = SortIntent::RequestClearSort { column_id: col_id };
    assert_eq!(clear.column_id(), col_id);
}

#[test]
fn test_edit_intent_extraction() {
    let col_id = ColumnId::new();
    let row_id = RowId::new();

    let begin = EditIntent::RequestBeginEdit {
        row_id,
        column_id: col_id,
    };
    assert_eq!(begin.column_id(), col_id);
    assert_eq!(begin.row_id(), row_id);

    let commit = EditIntent::RequestCommitEdit {
        row_id,
        column_id: col_id,
        payload: halcyon::widgets::tabular_view::EditPayload::String("test".to_string()),
    };
    assert_eq!(commit.column_id(), col_id);
    assert_eq!(commit.row_id(), row_id);

    let cancel = EditIntent::RequestCancelEdit {
        row_id,
        column_id: col_id,
    };
    assert_eq!(cancel.column_id(), col_id);
    assert_eq!(cancel.row_id(), row_id);
}

#[test]
fn test_sort_intent_with_direction() {
    let col_id = ColumnId::new();

    let intent = SortIntent::RequestSort {
        column_id: col_id,
        direction: Some(SortDirection::Descending),
    };

    if let SortIntent::RequestSort { direction, .. } = intent {
        assert_eq!(direction, Some(SortDirection::Descending));
    } else {
        panic!("Expected RequestSort");
    }
}

#[test]
fn test_sort_intent_toggle() {
    let col_id = ColumnId::new();

    let intent = SortIntent::RequestSort {
        column_id: col_id,
        direction: None, // Toggle
    };

    if let SortIntent::RequestSort { direction, .. } = intent {
        assert_eq!(direction, None);
    } else {
        panic!("Expected RequestSort");
    }
}

#[test]
fn test_edit_payload_variants() {
    use halcyon::widgets::tabular_view::EditPayload;

    let string_payload = EditPayload::String("hello".to_string());
    assert!(matches!(string_payload, EditPayload::String(_)));

    let number_payload = EditPayload::Number(42.0);
    assert!(matches!(number_payload, EditPayload::Number(_)));

    let bool_payload = EditPayload::Bool(true);
    assert!(matches!(bool_payload, EditPayload::Bool(_)));

    let none_payload = EditPayload::None;
    assert!(matches!(none_payload, EditPayload::None));

    let default_payload = EditPayload::default();
    assert!(matches!(default_payload, EditPayload::None));
}

#[test]
fn test_navigation_direction_variants() {
    use halcyon::widgets::tabular_view::NavigationDirection;

    // Just verify all variants exist and are distinct
    let directions = [
        NavigationDirection::Left,
        NavigationDirection::Right,
        NavigationDirection::Up,
        NavigationDirection::Down,
        NavigationDirection::Home,
        NavigationDirection::End,
        NavigationDirection::PageUp,
        NavigationDirection::PageDown,
    ];

    for (i, dir1) in directions.iter().enumerate() {
        for (j, dir2) in directions.iter().enumerate() {
            if i == j {
                assert_eq!(dir1, dir2);
            } else {
                assert_ne!(dir1, dir2);
            }
        }
    }
}
