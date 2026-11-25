// =============================================================================
// SPEC-PM-017 Rung 1a: f64 scroll core precision tests
// =============================================================================
//
// Proves the engine half of the PM-017 remedy at the exact depth that broke
// the SPEC-HAL-259 stress demo (content y past 2^24): the published window
// values are f64-exact and the first index is engine-published, not
// re-derived from f32 scroll arithmetic.
// =============================================================================

use crate::engines::hierarchical::{HierarchicalTabularSubstrate, NodeMeta};

fn deep_flat_hts(rows: usize) -> HierarchicalTabularSubstrate {
    let mut nodes = Vec::with_capacity(rows);
    for i in 0..rows {
        nodes.push(NodeMeta {
            node_id: i as u64,
            parent_index: None,
            depth: 0,
            has_children: false,
            label: String::new(),
        });
    }
    let mut hts = HierarchicalTabularSubstrate::new();
    hts.set_tree_data(nodes);
    hts.set_viewport_height(600.0);
    hts
}

/// Past 2^24 (row 699,051 at 24 px), f32 content y quantizes to a 2 px
/// lattice. The f64 window must be exact: first_y is the exact product,
/// and (first_y − scroll) is the exact sub-row phase.
#[test]
fn deep_scroll_window_is_f64_exact() {
    let mut hts = deep_flat_hts(800_000);
    let scroll: f64 = 16_777_300.0; // past 2^24 = 16,777,216
    hts.scroll_to(scroll);

    let window = hts.visible_window();
    let expected_first = (scroll / 24.0).floor() as usize; // 699,054
    assert_eq!(window.first_sorted, expected_first);
    assert_eq!(window.first_y, expected_first as f64 * 24.0);

    // Exact sub-row phase: 699,054 × 24 = 16,777,296 → phase = −4.0 exactly.
    let phase = window.first_y - hts.scroll_offset();
    assert_eq!(phase, -4.0);
    assert!(phase <= 0.0 && phase > -24.0);

    let mut stamp = hts.fixed_window_stamp().unwrap();
    let realized = window.first_sorted..window.first_sorted + window.rows.len();
    assert!(hts.scroll_within_fixed_window(scroll + 0.25, realized.clone(), &mut stamp));
    assert_eq!(window.first_y - hts.scroll_offset(), -4.25);
    assert!(!hts.scroll_within_fixed_window(window.first_y + 24.0, realized, &mut stamp));
    assert_eq!(hts.scroll_offset(), scroll + 0.25);
}

/// SPEC-PM-009 discharge: the first index has one source — the engine.
/// visible_rows_with_total and visible_window must agree, and the first
/// visible row must actually be that row.
#[test]
fn first_sorted_is_single_source() {
    let mut hts = deep_flat_hts(800_000);
    hts.scroll_to(16_777_300.0);

    let (rows, total, first) = hts.visible_rows_with_total();
    assert_eq!(total, 800_000);
    assert_eq!(first, hts.visible_window().first_sorted);
    assert_eq!(rows.first().expect("non-empty window").row_index, first);
}

#[test]
fn fixed_scroll_window_rejects_another_substrate_at_the_same_revision() {
    let mut old = deep_flat_hts(30);
    let window = old.visible_window();
    let mut stamp = old.fixed_window_stamp().unwrap();
    let realized = window.first_sorted..window.first_sorted + window.rows.len();

    let mut replacement = deep_flat_hts(30);
    replacement.visible_window();
    assert_eq!(old.revision(), replacement.revision());
    assert!(replacement.fixed_window_stamp().is_some());
    assert!(!replacement.scroll_within_fixed_window(1.25, realized, &mut stamp));
    assert_eq!(replacement.scroll_offset(), 0.0);
}
