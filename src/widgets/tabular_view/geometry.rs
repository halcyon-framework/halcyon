// =============================================================================
// SPEC-HAL-255: Compatibility shim — engine extracted to src/engines/column_geometry.rs
// =============================================================================
//
// All column geometry types now live in crate::engines::column_geometry.
// This module re-exports them for backward compatibility with existing consumers.
// =============================================================================

pub use crate::engines::column_geometry::{ColumnSlot, GeometryView, ResolvedColumnGeometry};

/// Compatibility alias: maps the legacy name to the extracted engine type.
pub type ColumnGeometryTable = crate::engines::column_geometry::ColumnGeometryEngine;
