// =============================================================================
// TabularSubstrate Integration Tests
// =============================================================================
//
// Tests exercising only public APIs.
//
// Tests requiring pub(crate) access (ColumnGeometryEngine::resolve, etc.)
// have been moved to src/tests/tabular_view/.
// =============================================================================

mod column_model;
mod edit_focus_separation;
mod focus_navigation;
mod intents;
mod row_identity_stability;
mod sort_wiring;
