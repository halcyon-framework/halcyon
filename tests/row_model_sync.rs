// =============================================================================
// RowModel ↔ RowHeights Sync Regression Test (INV-ROWMODEL-SYNC)
//
// Exercises every RowModel mutation and asserts rows/heights stay in sync
// after each operation. Also exercises read-path methods to ensure no
// debug_assert fires and behavior is consistent.
// =============================================================================

use halcyon::widgets::tabular_view::row::{RowId, RowModel};

fn assert_sync(rm: &RowModel) {
    assert_eq!(
        rm.rows().len(),
        rm.heights().len(),
        "RowModel/RowHeights desync"
    );
    assert_eq!(
        rm.len(),
        rm.heights().len(),
        "len() disagrees with heights().len()"
    );
}

fn exercise_read_paths(rm: &RowModel) {
    let len = rm.len();
    for i in 0..len {
        let _ = rm.height_at(i);
    }
    let _ = rm.total_height();
    if len > 0 {
        let _ = rm.y_offset_at(len - 1);
        let _ = rm.row_at_y(0.0);
    }
    let _ = rm.fixed_height();
    let _ = rm.height_version();
    let _ = rm.is_empty();
}

#[test]
fn variable_height_push_remove_clear_cycle() {
    let mut rm = RowModel::with_variable_heights();
    assert_sync(&rm);
    exercise_read_paths(&rm);

    // Push 10 rows
    for _ in 0..10 {
        rm.push(RowId::new());
        assert_sync(&rm);
    }
    assert_eq!(rm.len(), 10);
    exercise_read_paths(&rm);

    // push_with_height
    rm.push_with_height(RowId::new(), 40.0);
    assert_sync(&rm);
    assert_eq!(rm.len(), 11);
    assert_eq!(rm.height_at(10), 40.0);
    exercise_read_paths(&rm);

    // set_height on multiple indices
    rm.set_height(0, 30.0);
    assert_sync(&rm);
    assert_eq!(rm.height_at(0), 30.0);

    rm.set_height(5, 50.0);
    assert_sync(&rm);
    assert_eq!(rm.height_at(5), 50.0);

    rm.set_height(10, 60.0);
    assert_sync(&rm);
    assert_eq!(rm.height_at(10), 60.0);
    exercise_read_paths(&rm);

    // Remove from middle
    let removed = rm.remove(5);
    assert!(removed.is_some());
    assert_sync(&rm);
    assert_eq!(rm.len(), 10);
    exercise_read_paths(&rm);

    // Remove from end
    let removed = rm.remove(rm.len() - 1);
    assert!(removed.is_some());
    assert_sync(&rm);
    assert_eq!(rm.len(), 9);
    exercise_read_paths(&rm);

    // Remove out of bounds returns None
    let removed = rm.remove(100);
    assert!(removed.is_none());
    assert_sync(&rm);

    // Clear
    rm.clear();
    assert_sync(&rm);
    assert_eq!(rm.len(), 0);
    assert!(rm.is_empty());
    exercise_read_paths(&rm);
}

#[test]
fn fixed_height_push_remove_clear_cycle() {
    let mut rm = RowModel::with_fixed_height(24.0);
    assert_sync(&rm);

    for _ in 0..10 {
        rm.push(RowId::new());
        assert_sync(&rm);
    }
    assert_eq!(rm.len(), 10);
    assert_eq!(rm.height_at(0), 24.0);
    assert_eq!(rm.height_at(9), 24.0);
    assert_eq!(rm.total_height(), 240.0);
    exercise_read_paths(&rm);

    rm.remove(4);
    assert_sync(&rm);
    assert_eq!(rm.len(), 9);
    assert_eq!(rm.total_height(), 216.0);

    rm.set_fixed_height(30.0);
    assert_sync(&rm);
    assert_eq!(rm.height_at(0), 30.0);
    assert_eq!(rm.total_height(), 270.0);
    exercise_read_paths(&rm);

    rm.clear();
    assert_sync(&rm);
    assert_eq!(rm.len(), 0);
    exercise_read_paths(&rm);
}

#[test]
fn total_height_and_y_offset_consistency() {
    let mut rm = RowModel::with_variable_heights();
    let heights = [10.0, 20.0, 30.0, 40.0, 50.0];
    for &h in &heights {
        rm.push_with_height(RowId::new(), h);
    }
    assert_sync(&rm);

    // y_offset_at(i) == sum of heights[0..i]
    assert_eq!(rm.y_offset_at(0), 0.0);
    assert_eq!(rm.y_offset_at(1), 10.0);
    assert_eq!(rm.y_offset_at(2), 30.0);
    assert_eq!(rm.y_offset_at(3), 60.0);
    assert_eq!(rm.y_offset_at(4), 100.0);

    // total_height == sum of all
    assert_eq!(rm.total_height(), 150.0);

    // row_at_y returns correct row
    assert_eq!(rm.row_at_y(0.0), Some(0));
    assert_eq!(rm.row_at_y(9.9), Some(0));
    assert_eq!(rm.row_at_y(10.0), Some(1));
    assert_eq!(rm.row_at_y(29.9), Some(1));
    assert_eq!(rm.row_at_y(30.0), Some(2));
    assert_eq!(rm.row_at_y(149.9), Some(4));
    assert_eq!(rm.row_at_y(150.0), None);
    assert_eq!(rm.row_at_y(-1.0), None);
}

#[test]
fn identity_survives_mutations() {
    let mut rm = RowModel::with_fixed_height(24.0);
    let ids: Vec<RowId> = (0..5).map(|_| RowId::new()).collect();
    for &id in &ids {
        rm.push(id);
    }
    assert_sync(&rm);

    // Verify identity lookup
    for (i, &id) in ids.iter().enumerate() {
        assert_eq!(rm.get(i), Some(id));
        assert_eq!(rm.index_of(id), Some(i));
    }

    // Remove index 2, verify remaining identities shifted
    rm.remove(2);
    assert_sync(&rm);
    assert_eq!(rm.get(0), Some(ids[0]));
    assert_eq!(rm.get(1), Some(ids[1]));
    assert_eq!(rm.get(2), Some(ids[3]));
    assert_eq!(rm.get(3), Some(ids[4]));
    assert_eq!(rm.index_of(ids[2]), None);
}
