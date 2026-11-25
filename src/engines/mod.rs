// =============================================================================
// SPEC-HAL-255: Extracted Structural Engines
// =============================================================================
//
// Engine modules extracted from widget-specific substrate code.
// Engines own invariants and resolve logic; they have no widget, layout,
// or rendering dependencies.
// =============================================================================

pub mod cell_mechanics;
pub mod cell_source;
pub mod column_geometry;
pub mod hierarchical;
pub mod route_geometry;
pub mod row_topology;
pub mod temporal;
pub mod virtualization;
