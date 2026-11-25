use crate::widgets::tabular_view::{BodyDomain, GeometryView};

mod body_virtualization;
mod cell_content_projection;
mod column_config;
mod column_visibility;
mod geometry_resolution;
mod guards;
mod header_projection;
mod scroll_domain_partition;
mod selection;
mod separators;
mod sort_virtualization;

/// Test helper: realize viewport with sorted indices.
///
/// Replaces the removed `BodyDomain::realize_viewport_sorted` by computing
/// the visible range from scroll state and fixed row heights, then calling
/// `realize_with_range`.
fn test_realize_sorted(
    body: &mut BodyDomain,
    geometry: GeometryView<'_>,
    sorted_indices: &[usize],
) {
    if sorted_indices.is_empty() || body.row_model().is_empty() {
        body.realize_with_range(geometry, sorted_indices, 0, 0, 0.0);
        return;
    }
    let scroll_y = body.scroll_y();
    let viewport_height = body.viewport().height;
    let row_count = sorted_indices.len();

    // Accumulate heights in sorted order to find first/last visible
    let mut y = 0.0_f32;
    let mut first = row_count;
    let mut y_offset_at_first = 0.0_f32;
    let mut last_exclusive = row_count;
    for (i, &row_idx) in sorted_indices.iter().enumerate() {
        let h = body.row_model().height_at(row_idx);
        if first == row_count && y + h > scroll_y {
            first = i;
            y_offset_at_first = y - scroll_y;
        }
        if y >= scroll_y + viewport_height && last_exclusive == row_count {
            last_exclusive = i;
        }
        y += h;
    }
    if first == row_count {
        first = 0;
        last_exclusive = 0;
        y_offset_at_first = 0.0;
    }

    body.realize_with_range(
        geometry,
        sorted_indices,
        first,
        last_exclusive,
        y_offset_at_first,
    );
}

/// Test helper: realize viewport in natural (unsorted) order.
///
/// Replaces the removed `BodyDomain::realize_viewport`.
fn test_realize(body: &mut BodyDomain, geometry: GeometryView<'_>) {
    let n = body.row_model().len();
    let indices: Vec<usize> = (0..n).collect();
    test_realize_sorted(body, geometry, &indices);
}
