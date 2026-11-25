// =============================================================================
// SPEC-HAL-259: HierarchicalTabularSubstrate — Structural Engine Composition
// =============================================================================
//
// Substrate-level engine composing extracted engines for hierarchical tabular
// views. Owns orchestration only. Each engine owns its own invariants.
//
// Phase order: Topology → Geometry → Virtualization → Realization
//
// This is NOT a widget. No rendering, no UI events, no gesture handling.
//
// INV-HTS-001: No engine may mutate another engine's state.
// INV-HTS-002: Projection is deterministic given topology + geometry + scroll.
// INV-HTS-003: Collapsed subtrees occupy exactly one visible block.
// INV-HTS-004: Topology changes do not force column geometry recompute.
// INV-HTS-005: Invalidation follows SPEC-HAL-258 allowed flow.
// =============================================================================

pub(crate) mod topology;
pub(crate) mod virtualization;

use crate::core::ordered_selection::OrderedIndex;
use crate::engines::column_geometry::{ColumnGeometryEngine, GeometryView};
use crate::engines::row_topology::TopologyEngine;
use crate::engines::virtualization::VirtualizationStrategy;
use crate::model::tabular::column::{self, Column, ColumnId, ColumnModel, ColumnScrollDomain};
use crate::model::tabular::row_expansion::{ExpandedHeightPolicy, ExpansionPolicy, RowExpansion};
use crate::model::tabular::row_heights::RowHeights;

pub use topology::NodeMeta;

use topology::HierarchicalRowTopology;
use virtualization::HierarchicalBlockVirtualization;

/// Visible row in the hierarchical projection.
///
/// Returned by `visible_rows()`. Contains the physical row index plus
/// tree metadata for rendering tree guides, indentation, and toggles.
#[derive(Debug, Clone)]
pub struct VisibleRow {
    /// Caller-assigned stable node identifier.
    /// INV-NODE-ID-STABLE: Stable across scroll and expand/collapse.
    pub node_id: u64,
    /// Physical row index in the dataset.
    pub row_index: usize,
    /// Tree depth (0 for root nodes).
    pub depth: u32,
    /// Whether this node is expanded (children visible).
    pub expanded: bool,
    /// Whether this node has children.
    pub has_children: bool,
    /// Guide mask: bit `d` set means "draw vertical continuation line at depth d".
    pub guide_mask: u64,
    /// Stem-down flag: true when this node is expanded and has children
    /// (draw vertical stem down from this node at its depth level).
    pub stem_down: bool,
    /// Display label for this node.
    pub label: String,
}

/// Visible viewport window: the rows plus the content-space y of the
/// first one (SPEC-HAL-337 §5).
///
/// INV-337-R1: consumers derive per-row geometry exclusively from
/// substrate-published values; `first_y` is the virtualization's
/// y-offset, published instead of discarded.
#[derive(Debug, Clone)]
pub struct VisibleWindow {
    pub rows: Vec<VisibleRow>,
    /// Content-space y of `rows[0]` (the value `compute_visible_range`
    /// already produces), in f64 per SPEC-PM-017 INV-PM-017-2. `0.0`
    /// when `rows` is empty.
    pub first_y: f64,
    /// Engine-computed sorted index of `rows[0]` — the single source
    /// for consumers (SPEC-PM-017 / SPEC-PM-009: consumers must not
    /// re-derive this from an f32 scroll division). `0` when `rows`
    /// is empty.
    pub first_sorted: usize,
}

/// HierarchicalTabularSubstrate — structural engine composition.
///
/// SPEC-HAL-259: Composes extracted engines for hierarchical tabular views.
/// Owns orchestration only. Each engine owns its own invariants.
///
/// ## Composition (SPEC-HAL-259 §4)
///
/// - ColumnGeometryEngine (shared, SPEC-HAL-255)
/// - HierarchicalRowTopology (SPEC-HAL-259 §5.2, implements TopologyEngine)
/// - HierarchicalBlockVirtualization (SPEC-HAL-259 §5.3, implements VirtualizationStrategy)
/// - ScrollState (local)
/// - RowHeights (local, engine-grade height oracle)
///
/// ## Phase Order
///
/// Topology → Geometry → Virtualization → Realization
///
/// ## Non-Goals
///
/// No rendering API. No UI events. No gesture handling.
/// These belong to higher layers (SPEC-HAL-259 §13).
pub struct HierarchicalTabularSubstrate {
    // --- Engines ---
    geometry: ColumnGeometryEngine,
    topology: HierarchicalRowTopology,
    virtualization: HierarchicalBlockVirtualization,

    // --- Column model ---
    column_model: ColumnModel,

    // --- Row model (height queries for virtualization) ---
    // RowHeights is externally injected, not recreated by tree mutations.
    // For fixed-height mode (default), only the height value matters;
    // row count is derived from topology's sorted indices.
    row_heights: RowHeights,

    // --- Scroll state (SPEC-HAL-259 §5.5, amended by SPEC-PM-017
    // INV-PM-017-1: scroll authorities store f64) ---
    scroll_y: f64,
    viewport_height: f32,

    // --- Evaluation epoch (geometry epoch tracking) ---
    evaluation_epoch: u64,

    // --- Revision counter (combined change detection) ---
    revision_counter: u64,
    fixed_window_source: Option<std::sync::Arc<()>>,

    /// SPEC-HAL-313 §3.2: Row expansion state.
    ///
    /// INV-313-SUBSTRATE-OWNS-1: Expansion state is owned exclusively here.
    /// Distinct from tree expansion (expand/collapse of node children).
    /// Row expansion shows/hides a detail widget subtree for a row.
    row_expansion: RowExpansion,
}

/// Evidence from a completed fixed-height layout. The source marker distinguishes
/// replacement substrates even when a public shared handle and revision match.
pub(crate) struct FixedWindowStamp {
    source: std::sync::Arc<()>,
    revision: u64,
    geometry_epoch: u64,
    topology_version: u64,
}

/// SPEC-HAL-337 §2: the only ownership form for the topology substrate.
/// Handle clones share one instance (INV-337-O1).
pub type SharedHts = std::rc::Rc<std::cell::RefCell<HierarchicalTabularSubstrate>>;

/// SPEC-HAL-337 §2 policy type: who constructed the topology this widget
/// projects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HtsOwnership {
    /// Widget minted its own substrate at construction and exposes the handle.
    Internal,
    /// Substrate was injected; the application (or another composer) owns
    /// construction.
    External,
}

impl HierarchicalTabularSubstrate {
    /// Create a new HierarchicalTabularSubstrate with default configuration.
    pub fn new() -> Self {
        Self {
            geometry: ColumnGeometryEngine::new(),
            topology: HierarchicalRowTopology::new(),
            virtualization: HierarchicalBlockVirtualization::new(),
            column_model: ColumnModel::new(),
            row_heights: RowHeights::with_fixed_height(24.0),
            scroll_y: 0.0,
            viewport_height: 0.0,
            evaluation_epoch: 0,
            revision_counter: 0,
            fixed_window_source: None,
            row_expansion: RowExpansion::default(),
        }
    }

    // =========================================================================
    // SPEC-HAL-259 §8: Public API Surface
    // =========================================================================

    /// Set tree data (topology adapter).
    ///
    /// SPEC-HAL-259 §8: `set_topology_adapter(...)`.
    /// Replaces the entire tree structure. Each row gets a NodeMeta
    /// describing its position in the hierarchy.
    ///
    /// Does NOT recreate or mutate the RowHeights. Tree metadata describes
    /// structure over a pre-existing dataset; the row model is injected
    /// separately via `set_row_heights()` or `set_row_height()`.
    ///
    /// Invalidation flows through engine-owned gating in
    /// HierarchicalRowTopology (topology_dirty + tree_dirty).
    pub fn set_tree_data(&mut self, nodes: Vec<NodeMeta>) {
        self.topology.set_tree_data(nodes);
        self.enforce_row_count_invariant();
        self.revision_counter += 1;
    }

    /// Set the row heights (external dataset injection).
    ///
    /// SPEC-HAL-259: Tree metadata describes structure over a pre-existing
    /// dataset. Row heights provide height queries for virtualization.
    /// Caller is responsible for ensuring row count alignment with tree
    /// data when using variable-height mode.
    ///
    /// For fixed-height mode (default), only the height value matters;
    /// row count is derived from topology's sorted indices.
    pub fn set_row_heights(&mut self, row_heights: RowHeights) {
        self.row_heights = row_heights;
        self.enforce_row_count_invariant();
        self.revision_counter += 1;
    }

    /// Set column model.
    ///
    /// SPEC-HAL-259 §8: `set_columns(...)`.
    /// Replaces the column configuration used by ColumnGeometryEngine.
    pub fn set_columns(&mut self, model: ColumnModel) {
        self.column_model = model;
        self.geometry.invalidate();
        self.revision_counter += 1;
    }

    /// Set a single column's preferred width.
    ///
    /// Mutates the column model in-place (no full model replacement).
    /// Invalidates geometry so next `resolve_geometry()` recomputes slots.
    ///
    /// INV-MIN-WIDTH: Clamps to max(col.min_width, MIN_COL_WIDTH) floor and
    /// col.max_width ceiling. Mirrors TabularSubstrate::handle_resize_intent.
    pub fn set_column_width(&mut self, id: ColumnId, width: f32) {
        let Some(col) = self.column_model.get(id) else {
            return;
        };
        let floor = col.min_width.max(column::MIN_COL_WIDTH);
        let new_width = width.max(floor).min(col.max_width);
        if self.column_model.set_preferred_width(id, new_width) {
            self.geometry.invalidate();
            self.revision_counter += 1;
        }
    }

    /// Set or clear a column's flex weight.
    ///
    /// `flex = Some(weight)` marks the column as elastic with the given
    /// flex weight; the resolved layout distributes remaining width to
    /// flex columns proportionally. `flex = None` clears elasticity —
    /// the column reverts to its `preferred_width`.
    ///
    /// SPEC-HAL-180 INV-COL-ELASTIC-1 (at most one elastic column per
    /// layout) is NOT enforced here — this method sets a single
    /// column's flex weight in isolation. Callers that need the
    /// invariant (e.g. `PropertyEditor::set_column_elastic`) clear
    /// flex on every other registered column before setting the
    /// target.
    ///
    /// SPEC-HAL-165 INV-1: column-geometry mutation belongs to the
    /// substrate. Widgets route flex changes through this method —
    /// they do not reach into the column model directly.
    ///
    /// Invalidates cached geometry and bumps the revision counter so
    /// observers (PE / TreeView body) re-resolve on the next layout
    /// pass.
    pub fn set_column_flex(&mut self, id: ColumnId, flex: Option<f32>) {
        if self.column_model.set_flex(id, flex) {
            self.geometry.invalidate();
            self.revision_counter += 1;
        }
    }

    /// Set or clear the content-derived width floor for an elastic column.
    ///
    /// Forwards to `ColumnModel::set_content_width`. Invalidates cached
    /// geometry and bumps the revision counter so observers re-resolve
    /// on the next layout pass — mirrors `set_column_flex` exactly.
    ///
    /// SPEC-HAL-165 INV-1: column-geometry mutation belongs to the
    /// substrate. Widgets that derive content widths from their data
    /// source (e.g. PropertyEditor) route through this method.
    pub fn set_column_content_width(&mut self, id: ColumnId, width: Option<f32>) {
        if self.column_model.set_content_width(id, width) {
            self.geometry.invalidate();
            self.revision_counter += 1;
        }
    }

    /// Apply a user-initiated column resize via the model authority.
    ///
    /// Wraps `ColumnModel::commit_user_resize` with HTS-side invalidation:
    /// invalidates cached geometry and bumps the revision counter so dependent
    /// observers (PE / TreeView body) re-resolve on the next layout pass.
    ///
    /// SPEC-HAL-260A §0 (Data-Layer Sovereignty): column geometry is View
    /// domain owned by HTS. Widgets route user-resize gestures through this
    /// method — they do not reach into the column model.
    ///
    /// SPEC-HAL-165 INV-1: column-geometry mutation belongs to the substrate.
    /// The contract (clamp, clear elastic, set preferred_width) is enforced
    /// by `ColumnModel::commit_user_resize`; HTS snapshots frame-resolved
    /// elastic siblings before the target mutation so the drag cannot reflow
    /// unrelated columns through flex redistribution.
    pub fn commit_user_resize(&mut self, id: ColumnId, proposed_width: f32) -> Option<f32> {
        if !self
            .column_model
            .get(id)
            .is_some_and(|column| column.resizable)
        {
            return None;
        }

        let froze_elastic = self.freeze_elastic_widths_for_resize(id);
        let resized = self.column_model.commit_user_resize(id, proposed_width);
        let applied_width = self
            .column_model
            .get(id)
            .map(|column| column.preferred_width);

        if froze_elastic || resized {
            self.geometry.invalidate();
            self.revision_counter += 1;
        }

        applied_width
    }

    fn freeze_elastic_widths_for_resize(&mut self, target_column_id: ColumnId) -> bool {
        if !self.geometry.is_resolved_for_epoch(self.evaluation_epoch) {
            return false;
        }

        let widths = self
            .geometry
            .header_slots(self.evaluation_epoch)
            .filter_map(|slot| {
                if slot.column_id == target_column_id {
                    return None;
                }
                self.column_model.get(slot.column_id).and_then(|column| {
                    column
                        .flex
                        .is_some()
                        .then_some((slot.column_id, slot.width))
                })
            })
            .collect::<Vec<_>>();

        self.column_model.freeze_elastic_widths(widths)
    }

    /// Append a column to the column model.
    ///
    /// Mirrors `set_columns` (full replacement) and `set_column_width`
    /// (single property mutation) — the column-model mutation pattern.
    /// Pushes `column` at the end of the model, invalidates cached
    /// geometry so the next `resolve_geometry()` recomputes slots, and
    /// bumps the revision counter so observers re-read.
    ///
    /// Returns the appended column's `ColumnId` (taken from `column.id`)
    /// for caller convenience. Consumers that store column IDs in
    /// ordered metadata — e.g. PE Core's `value_column_ids: Vec<ColumnId>`
    /// per SPEC-HAL-260A §8.1 — chain directly:
    ///
    /// ```ignore
    /// let col_id = hts.add_column(Column::new(120.0).with_flex(1.0));
    /// self.value_column_ids.push(col_id);
    /// ```
    ///
    /// SPEC-HAL-165 INV-1 / INV-3: Column Model is owned exclusively by
    /// the substrate; widgets do not create columns directly. This
    /// method is the substrate-side seam for column append.
    ///
    /// SPEC-HAL-165 INV-4 / INV-5 / INV-6: Geometry is not resolved
    /// here. Cached geometry is invalidated; the next layout pass
    /// performs resolution.
    pub fn add_column(&mut self, column: Column) -> ColumnId {
        let id = column.id;
        self.column_model.push(column);
        self.geometry.invalidate();
        self.revision_counter += 1;
        id
    }

    /// Set a column's visibility.
    pub fn set_column_visible(&mut self, id: ColumnId, visible: bool) -> bool {
        if self.column_model.set_visible(id, visible) {
            self.geometry.invalidate();
            self.revision_counter += 1;
            true
        } else {
            false
        }
    }

    /// Toggle a column's visibility.
    pub fn toggle_column_visible(&mut self, id: ColumnId) -> bool {
        let Some(current) = self.column_model.get(id).map(|c| c.visible) else {
            return false;
        };
        self.set_column_visible(id, !current)
    }

    /// Query a column's visibility.
    pub fn is_column_visible(&self, id: ColumnId) -> bool {
        self.column_model
            .get(id)
            .map(|c| c.visible)
            .unwrap_or(false)
    }

    /// Set a column's scroll domain.
    pub fn set_column_scroll_domain(&mut self, id: ColumnId, domain: ColumnScrollDomain) -> bool {
        if self.column_model.set_scroll_domain(id, domain) {
            self.geometry.invalidate();
            self.revision_counter += 1;
            true
        } else {
            false
        }
    }

    /// Reorder columns according to a complete column-id order.
    pub fn reorder_columns(&mut self, new_order: &[ColumnId]) -> bool {
        if self.column_model.reorder(new_order) {
            self.geometry.invalidate();
            self.revision_counter += 1;
            true
        } else {
            false
        }
    }

    /// Set uniform row height.
    ///
    /// SPEC-HAL-259 §8: `set_row_height(...)`.
    pub fn set_row_height(&mut self, height: f32) {
        self.row_heights.set_fixed_height(height);
        self.revision_counter += 1;
    }

    /// Expand a tree node (make its children visible).
    ///
    /// SPEC-HAL-259 §8: `expand(node_id)`.
    /// Invalidation flows through engine-owned gating (topology_dirty).
    pub fn expand(&mut self, row_index: usize) {
        self.topology.expand(row_index);
        self.revision_counter += 1;
    }

    /// Collapse a tree node (hide its children).
    ///
    /// SPEC-HAL-259 §8: `collapse(node_id)`.
    pub fn collapse(&mut self, row_index: usize) {
        self.topology.collapse(row_index);
        self.revision_counter += 1;
    }

    /// Toggle a tree node's expansion state.
    ///
    /// SPEC-HAL-259 §8: `toggle(node_id)`.
    pub fn toggle(&mut self, row_index: usize) {
        self.topology.toggle(row_index);
        self.revision_counter += 1;
    }

    /// Expand by stable node_id. No-op if id not found.
    pub fn expand_by_id(&mut self, node_id: u64) {
        if let Some(index) = self.topology.index_for_id(node_id) {
            self.topology.expand(index);
            self.revision_counter += 1;
        }
    }

    /// Collapse by stable node_id. No-op if id not found.
    pub fn collapse_by_id(&mut self, node_id: u64) {
        if let Some(index) = self.topology.index_for_id(node_id) {
            self.topology.collapse(index);
            self.revision_counter += 1;
        }
    }

    /// Toggle by stable node_id. No-op if id not found.
    pub fn toggle_by_id(&mut self, node_id: u64) {
        if let Some(index) = self.topology.index_for_id(node_id) {
            self.topology.toggle(index);
            self.revision_counter += 1;
        }
    }

    /// Recursively expand a node and all its descendants.
    ///
    /// Revision only bumped when topology actually changed.
    pub fn expand_subtree(&mut self, row_index: usize) {
        if self.topology.expand_subtree(row_index) {
            self.revision_counter += 1;
        }
    }

    /// Recursively collapse a node and all its descendants.
    ///
    /// Revision only bumped when topology actually changed.
    pub fn collapse_subtree(&mut self, row_index: usize) {
        if self.topology.collapse_subtree(row_index) {
            self.revision_counter += 1;
        }
    }

    /// Recursively expand by stable node_id. No-op if id not found.
    /// Returns true if any node was expanded.
    pub fn expand_subtree_by_id(&mut self, node_id: u64) -> bool {
        if let Some(index) = self.topology.index_for_id(node_id) {
            if self.topology.expand_subtree(index) {
                self.revision_counter += 1;
                return true;
            }
        }
        false
    }

    /// Recursively collapse by stable node_id. No-op if id not found.
    /// Returns true if any node was collapsed.
    pub fn collapse_subtree_by_id(&mut self, node_id: u64) -> bool {
        if let Some(index) = self.topology.index_for_id(node_id) {
            if self.topology.collapse_subtree(index) {
                self.revision_counter += 1;
                return true;
            }
        }
        false
    }

    /// Set scroll offset.
    ///
    /// SPEC-HAL-259 §8: `scroll_to(offset)`.
    /// INV-HTS-005: Scroll → Virtualization only. No topology/geometry rebuild.
    pub fn scroll_to(&mut self, offset: f64) {
        self.scroll_y = offset;
        self.revision_counter += 1;
    }

    pub(crate) fn fixed_window_stamp(&mut self) -> Option<FixedWindowStamp> {
        let height = self.row_heights.fixed_height()?;
        if !height.is_finite() || height <= 0.0 || self.topology.is_topology_dirty() {
            return None;
        }
        Some(FixedWindowStamp {
            source: self
                .fixed_window_source
                .get_or_insert_with(|| std::sync::Arc::new(()))
                .clone(),
            revision: self.revision_counter,
            geometry_epoch: self.evaluation_epoch,
            topology_version: self.topology.topology_version(),
        })
    }

    /// Reuse an existing realization only when this substrate's own window
    /// calculation yields the same range with unchanged layout inputs.
    pub(crate) fn scroll_within_fixed_window(
        &mut self,
        offset: f64,
        realized: std::ops::Range<usize>,
        stamp: &mut FixedWindowStamp,
    ) -> bool {
        if stamp.revision != self.revision_counter
            || stamp.geometry_epoch != self.evaluation_epoch
            || stamp.topology_version != self.topology.topology_version()
            || self.topology.is_topology_dirty()
            || !self
                .fixed_window_source
                .as_ref()
                .is_some_and(|source| std::sync::Arc::ptr_eq(source, &stamp.source))
        {
            return false;
        }
        let (first, last, _) = self.virtualization.compute_visible_range(
            offset,
            offset + self.viewport_height as f64,
            &self.row_heights,
            self.topology.sorted_indices(),
            self.topology.topology_version(),
        );
        if (first..last) != realized {
            return false;
        }
        self.scroll_to(offset);
        stamp.revision = self.revision_counter;
        true
    }

    /// Query viewport height.
    pub fn viewport_height(&self) -> f32 {
        self.viewport_height
    }

    /// Set viewport height (required for virtualization).
    pub fn set_viewport_height(&mut self, height: f32) {
        self.viewport_height = height;
        self.revision_counter += 1;
    }

    /// Query the visible viewport window: rows plus the content-space y
    /// of the first row (SPEC-HAL-337 §5).
    ///
    /// Orchestration: ensures topology is current, then computes visible range
    /// via virtualization strategy, then builds VisibleRow entries.
    ///
    /// Phase order: Topology → Virtualization (geometry not needed for row queries).
    pub fn visible_window(&mut self) -> VisibleWindow {
        self.enforce_row_count_invariant();

        // Phase 1: Ensure topology is current.
        // row_count is node_count; HTS forbids RowHeights/tree divergence by invariant.
        let row_count = self.topology.node_count();
        let no_sort = |_: &mut [usize]| {};
        self.topology
            .ensure_sorted_order(row_count, None, false, &no_sort);

        // Phase 2: Compute visible range via virtualization.
        let sorted = self.topology.sorted_indices();
        let stamp = self.topology.topology_version();
        let viewport_bottom = self.scroll_y + self.viewport_height as f64;
        let (first, last, y_offset) = self.virtualization.compute_visible_range(
            self.scroll_y,
            viewport_bottom,
            &self.row_heights,
            sorted,
            stamp,
        );

        // Phase 3: Build visible row entries.
        let mut rows = Vec::with_capacity(last - first);
        for i in first..last {
            let row_index = sorted[i];
            rows.push(VisibleRow {
                node_id: self.topology.node_id(row_index),
                row_index,
                depth: self.topology.depth(row_index),
                expanded: self.topology.is_expanded(row_index),
                has_children: self.topology.has_children(row_index),
                guide_mask: self.topology.guide_mask(row_index),
                stem_down: self.topology.stem_down(row_index),
                label: self.topology.label(row_index).to_owned(),
            });
        }
        VisibleWindow {
            rows,
            first_y: y_offset,
            first_sorted: first,
        }
    }

    /// Query visible rows in the current viewport.
    ///
    /// SPEC-HAL-259 §8: `visible_rows() -> &[VisibleRow]`.
    /// Thin projection over `visible_window()` (SPEC-HAL-337 §5) — one
    /// windowing orchestration, no second copy.
    pub fn visible_rows(&mut self) -> Vec<VisibleRow> {
        self.visible_window().rows
    }

    /// Query visible rows in the current viewport, plus total visible row count
    /// and the engine-computed first sorted index.
    ///
    /// Single topology evaluation — no second pass needed.
    /// Returns `(viewport_visible_rows, total_sorted_count, first_sorted)`.
    /// SPEC-PM-017 / SPEC-PM-009: `first_sorted` is the single source;
    /// consumers must not re-derive it from scroll arithmetic.
    pub fn visible_rows_with_total(&mut self) -> (Vec<VisibleRow>, usize, usize) {
        self.enforce_row_count_invariant();

        // Phase 1: Ensure topology is current (SINGLE evaluation).
        let row_count = self.topology.node_count();
        let no_sort = |_: &mut [usize]| {};
        self.topology
            .ensure_sorted_order(row_count, None, false, &no_sort);

        // Read total BEFORE windowing.
        let sorted = self.topology.sorted_indices();
        let total_sorted_count = sorted.len();

        // Phase 2: Compute visible range via virtualization.
        let stamp = self.topology.topology_version();
        let viewport_bottom = self.scroll_y + self.viewport_height as f64;
        let (first, last, _y_offset) = self.virtualization.compute_visible_range(
            self.scroll_y,
            viewport_bottom,
            &self.row_heights,
            sorted,
            stamp,
        );

        // Phase 3: Build visible row entries.
        let mut result = Vec::with_capacity(last - first);
        for i in first..last {
            let row_index = sorted[i];
            result.push(VisibleRow {
                node_id: self.topology.node_id(row_index),
                row_index,
                depth: self.topology.depth(row_index),
                expanded: self.topology.is_expanded(row_index),
                has_children: self.topology.has_children(row_index),
                guide_mask: self.topology.guide_mask(row_index),
                stem_down: self.topology.stem_down(row_index),
                label: self.topology.label(row_index).to_owned(),
            });
        }
        (result, total_sorted_count, first)
    }

    /// Resolve column geometry for a given available width.
    ///
    /// Must be called before `column_layout()`. Advances evaluation epoch.
    ///
    /// INV-HTS-004: Independent of topology. Column resize does not
    /// trigger topology rebuild.
    pub fn resolve_geometry(&mut self, available_width: f32, scale_factor: f32) {
        self.evaluation_epoch += 1;
        if self
            .geometry
            .needs_resolve(&self.column_model, available_width, scale_factor)
        {
            self.geometry.resolve(
                &self.column_model,
                available_width,
                self.evaluation_epoch,
                scale_factor,
            );
        } else {
            self.geometry.stamp_epoch(self.evaluation_epoch);
        }
    }

    /// Query column layout.
    ///
    /// SPEC-HAL-259 §8: `column_layout() -> &[ColumnLayout]`.
    /// Returns an epoch-safe read-only view of resolved column geometry.
    ///
    /// Requires `resolve_geometry()` to have been called this evaluation.
    pub fn column_layout(&self) -> GeometryView<'_> {
        GeometryView::new(&self.geometry, self.evaluation_epoch)
    }

    /// Non-panicking geometry read gate for event-path code.
    ///
    /// Returns geometry only if resolved for the current evaluation epoch.
    /// Returns None if geometry was invalidated (e.g. by set_column_width)
    /// and not yet re-resolved by the next layout pass.
    pub fn try_column_layout(&self) -> Option<GeometryView<'_>> {
        if self.geometry.is_resolved_for_epoch(self.evaluation_epoch) {
            Some(GeometryView::new(&self.geometry, self.evaluation_epoch))
        } else {
            None
        }
    }

    /// Query current scroll offset.
    ///
    /// INV-TV-5: HTS is the single scroll authority.
    pub fn scroll_offset(&self) -> f64 {
        self.scroll_y
    }

    /// Query row height.
    ///
    /// Returns the fixed row height used for layout and virtualization.
    pub fn row_height(&self) -> f32 {
        self.row_heights.fixed_height().unwrap_or(24.0)
    }

    /// Query revision counter.
    ///
    /// SPEC-HAL-259 §8: `revision()`.
    /// Monotonically increasing counter bumped on any state mutation.
    pub fn revision(&self) -> u64 {
        self.revision_counter
    }

    /// Publish one provider-confirmed replacement of the flat base order.
    pub fn replace_base_order_revision(&mut self) {
        self.topology.increment_base_epoch();
        self.revision_counter += 1;
    }

    /// Current base-order epoch for revision-coherence probes.
    pub fn base_row_order_epoch(&self) -> u64 {
        self.topology.base_row_order_epoch()
    }

    // =========================================================================
    // SPEC-HAL-313: Row Expansion Configuration
    // =========================================================================

    /// SPEC-HAL-313 §3.2: Read-only access to expansion state.
    ///
    /// Row expansion is distinct from tree expansion. Tree expansion
    /// (expand/collapse/toggle methods above) controls child node visibility.
    /// Row expansion controls whether a detail widget subtree is shown for a row.
    pub fn expansion(&self) -> &RowExpansion {
        &self.row_expansion
    }

    /// SPEC-HAL-313 §3.2: Mutable access to expansion state.
    pub fn expansion_mut(&mut self) -> &mut RowExpansion {
        &mut self.row_expansion
    }

    /// SPEC-HAL-313 §3.2: Set expansion policy.
    pub fn set_expansion_policy(&mut self, policy: ExpansionPolicy) {
        self.row_expansion.set_policy(policy);
    }

    /// SPEC-HAL-313 §3.2: Set expanded height policy.
    pub fn set_expanded_height_policy(&mut self, policy: ExpandedHeightPolicy) {
        self.row_expansion.set_height_policy(policy);
    }

    // =========================================================================
    // Invariant enforcement
    // =========================================================================

    /// INVARIANT-HTS-ROW-COUNT: row_heights.len() == topology.node_count()
    /// whenever tree is active.
    fn enforce_row_count_invariant(&self) {
        let node_count = self.topology.node_count();
        if node_count == 0 {
            return; // Case A: tree inactive — no constraint.
        }
        let row_len = self.row_heights.len();
        let has_fixed = self.row_heights.fixed_height().is_some();
        if row_len == 0 && has_fixed {
            return; // Case B: zero-len fixed-height RowHeights — row count derived from topology.
        }
        if row_len != node_count {
            Self::row_count_invariant_fail(node_count, row_len, has_fixed);
        }
    }

    #[cold]
    #[inline(never)]
    fn row_count_invariant_fail(node_count: usize, row_len: usize, has_fixed_height: bool) -> ! {
        panic!(
            "INVARIANT-HTS-ROW-COUNT: node_count={}, row_heights.len()={}, fixed_height={}",
            node_count, row_len, has_fixed_height
        );
    }

    // =========================================================================
    // Internal accessors (for engine queries, not public API)
    // =========================================================================

    /// Access topology engine (immutable).
    pub(crate) fn topology(&self) -> &HierarchicalRowTopology {
        &self.topology
    }

    /// Access topology engine (mutable).
    #[allow(dead_code, reason = "Retained internal topology-owner access")]
    pub(crate) fn topology_mut(&mut self) -> &mut HierarchicalRowTopology {
        &mut self.topology
    }

    /// Access geometry engine (immutable).
    #[allow(dead_code, reason = "Retained immutable geometry-owner query")]
    pub(crate) fn geometry(&self) -> &ColumnGeometryEngine {
        &self.geometry
    }

    /// Read-only access to the underlying column model. Consumers iterate
    /// column properties (flex, content_width, IDs) for measurement and
    /// introspection. All mutation continues to flow through HTS
    /// mutators (SPEC-HAL-165 INV-1).
    pub fn column_model(&self) -> &ColumnModel {
        &self.column_model
    }
}

impl Default for HierarchicalTabularSubstrate {
    fn default() -> Self {
        Self::new()
    }
}

/// OrderedIndex<u64> impl for HTS — selection oracle using node_id as key.
///
/// Ordering source: `topology.sorted_indices()` — same ordering used by
/// `visible_rows_with_total()` (full projection, not viewport-windowed).
///
/// All trait methods are allocation-free:
/// - `len()`: slice len
/// - `key_at()`: two array lookups (sorted_indices[index] → node_id)
/// - `index_of()`: HashMap O(1) for physical index, then O(n) linear scan
///   of sorted_indices for sorted position (no reverse map exists; same
///   fallback pattern as ListView's RowHeights impl)
impl OrderedIndex<u64> for HierarchicalTabularSubstrate {
    fn len(&self) -> usize {
        self.topology.sorted_indices().len()
    }

    fn key_at(&self, index: usize) -> u64 {
        let row_index = self.topology.sorted_indices()[index];
        self.topology.node_id(row_index)
    }

    fn index_of(&self, key: u64) -> Option<usize> {
        // O(1) HashMap lookup: node_id → physical row_index
        let physical = self.topology.index_for_id(key)?;
        // O(n) linear scan: find sorted position of physical row_index
        self.topology
            .sorted_indices()
            .iter()
            .position(|&idx| idx == physical)
    }
}
