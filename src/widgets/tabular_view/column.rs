// =============================================================================
// SPEC-HAL-165: Column types — re-exported from model layer
// =============================================================================
//
// Column types now live in crate::model::tabular::column.
// This module re-exports them for backward compatibility with widget consumers.
// Engines import directly from crate::model::tabular::column.
// =============================================================================

pub use crate::model::tabular::column::*;
