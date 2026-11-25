// =============================================================================
// SPEC-HAL-187/188/189: ListView — CellMechanicsSubstrate Consumer
// =============================================================================
//
// STRUCTURALLY COMPLETE — NO NEW FEATURES
//
// ListView consumes CellMechanicsSubstrate and its composed HTS for column
// geometry, row virtualization, row expansion, and realized cell paint.
// ListView owns the DataAdapter bridge, row identity projection, and widget
// interaction policy.
//
// ## What ListView OWNS
//
// - A CellMechanicsSubstrate instance (CMS + HTS)
// - DataAdapter projection from SPEC-HAL-188 values into realized CMS cells
// - OrderedSelectionState for flat row selection/focus
// - Row widget hosting (creation/rebind keyed by RowId)
// - Cell widget hosting (creation/rebind keyed by (RowId, ColumnId))
// - Viewport wiring to HTS
//
// ## What ListView does NOT OWN (SPEC-HAL-187/188 contracts)
//
// - Row identity (owned by data layer via RowId)
// - Row order (owned by data layer, projected through adapter)
// - Column geometry (owned by HTS)
// - Cell paint dispatch (owned by CMS)
// - Data (owned by DataSource via adapter)
// - Permissions (owned by data layer via CellCapabilities)
//
// ## Forbidden Patterns (SPEC-HAL-187/189 Laws)
//
// - INV-ROW-BASE-REPLACE: No index-based row identity across epochs
// - INV-ROW-INDEX-EPHEMERAL: No persisted RowId↔index mappings
// - INV-189-RECREATION: No widget recreation outside identity death
// - INV-189-FORBIDDEN: No recreation on scroll, reorder, visibility, value/policy change
// - No `selected_index()`, `row_at_index()`, `scroll_to_index()` helpers
// - No caching of "visible rows" across frames
// - No assumption of contiguous, finite, or in-memory data
//
// =============================================================================

mod actions;
mod body;
mod cell_host;
pub mod cell_painter;
mod column_config_overlay;
mod core;
mod footer;
mod list_view;
mod row_host;

pub use actions::ListViewAction;
pub use body::ListViewBody;
pub use cell_host::{CellHost, CellKey, CellWidgetState};
pub use cell_painter::{CellPainter, DefaultCellPainter};
pub use core::{ListViewCore, ResizeState, RowId};
pub use footer::ListViewFooter;
pub use list_view::ListView;
#[cfg(test)]
pub(crate) use list_view::{
    ListViewColumnProbeEntry, ListViewRealizedCellProbeEntry, ListViewStructuralProbeEntry,
};
pub use row_host::{DirtyFlags, DirtyOutcome, RowHost, RowWidgetState, UpdatePhase};
