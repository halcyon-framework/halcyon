// SPEC-HAL-337 §5 / FORBID-337-2 adapter tests: hts_rows projection.
// INV-337-R1: geometry derives exclusively from the substrate's published
// surface. INV-HTSROWS-1: fixed-height contract (24.0 via the TreeView
// harness). Coordinate contract: content-space first_y minus scroll_offset
// = viewport-space y.

use crate::engines::hierarchical::NodeMeta;
use crate::engines::temporal::hts_rows::hts_rows;
use crate::widgets::tree_view::TreeView;

fn flat_nodes(n: usize) -> Vec<NodeMeta> {
    (0..n)
        .map(|i| NodeMeta {
            node_id: (i + 1) as u64,
            parent_index: None,
            depth: 0,
            has_children: false,
            label: format!("N{}", i + 1),
        })
        .collect()
}

#[test]
fn unscrolled_rows_map_to_viewport_space() {
    let view = TreeView::new();
    let h = view.hts_handle();
    h.borrow_mut().set_tree_data(flat_nodes(10));
    h.borrow_mut().set_viewport_height(50.0);

    let rows = hts_rows(&h);

    assert!(
        rows.len() >= 3,
        "50px viewport at 24px rows shows >= 3 rows"
    );
    for (i, r) in rows.iter().enumerate() {
        assert_eq!(r.id, (i + 1) as u64);
        assert_eq!(r.y, i as f32 * 24.0);
        assert_eq!(r.height, 24.0);
    }
}

#[test]
fn scrolled_first_row_is_partially_above() {
    let view = TreeView::new();
    let h = view.hts_handle();
    h.borrow_mut().set_tree_data(flat_nodes(10));
    h.borrow_mut().set_viewport_height(50.0);
    h.borrow_mut().scroll_to(50.0);

    let rows = hts_rows(&h);

    // scroll 50 at height 24: first visible index 2, content y 48,
    // viewport y 48 - 50 = -2 — inside (-height, 0].
    assert_eq!(rows[0].id, 3);
    assert_eq!(rows[0].y, -2.0);
    assert_eq!(rows[0].height, 24.0);
    assert_eq!(rows[1].y, 22.0);
}

#[test]
fn empty_topology_yields_free_mode_empty_vec() {
    let view = TreeView::new();
    let h = view.hts_handle();
    h.borrow_mut().set_viewport_height(50.0);

    let rows = hts_rows(&h);

    assert!(rows.is_empty());
}
