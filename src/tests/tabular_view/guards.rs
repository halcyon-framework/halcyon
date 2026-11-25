// =============================================================================
// TabularSubstrate Guards Integration Tests
// =============================================================================
//
// Tests for SPEC-HAL-164/165/168: Invariant enforcement.
//
// Note: Guard internals (FramePhaseGuard, ColumnModelGuard, RowModelGuard)
// are implementation details not exposed via public API. These tests verify
// observable invariant enforcement behavior through TabularSubstrate's public API.
//
// The internal guard unit tests that cannot be replicated via public API:
// - test_frame_phase_transitions (internal state machine)
// - test_frame_epoch_increments (internal counter)
// - test_column_model_guard (internal lock mechanism)
//
// These behaviors are implicitly tested through geometry resolution tests
// and TabularSubstrate layout/paint/event phase tests.
// =============================================================================

use crate::widgets::tabular_view::{Column, ColumnGeometryTable, ColumnModel};

// The following tests verify invariant enforcement that IS observable
// through public APIs:

#[test]
#[should_panic(expected = "INVARIANT-5")]
fn test_double_geometry_resolution_panics() {
    // SPEC-HAL-168 INVARIANT-5: Re-resolution within same frame is illegal
    let mut model = ColumnModel::new();
    model.push(Column::new(100.0));

    let mut table = ColumnGeometryTable::new();
    table.resolve(&model, 100.0, 1, 1.0);
    table.resolve(&model, 100.0, 1, 1.0); // Should panic - same frame
}

#[test]
#[should_panic(expected = "INVARIANT-6")]
fn test_geometry_read_before_resolution_panics() {
    // SPEC-HAL-165 INVARIANT-6: Cannot read geometry before resolution
    let table = ColumnGeometryTable::new();
    let _ = table.columns(0); // Should panic
}

#[test]
fn test_geometry_resolution_resets_between_frames() {
    // Verify that invalidation allows re-resolution in new frame
    let mut model = ColumnModel::new();
    model.push(Column::new(100.0));

    let mut table = ColumnGeometryTable::new();

    // Frame 1
    table.resolve(&model, 100.0, 1, 1.0);
    assert!(table.is_resolved());

    // Invalidate for new frame
    table.invalidate();
    assert!(!table.is_resolved());

    // Frame 2 - should be allowed
    table.resolve(&model, 100.0, 2, 1.0);
    assert!(table.is_resolved());
}

#[test]
fn test_staleness_detection() {
    // SPEC-HAL-168 INVARIANT-7: Stale geometry detection
    let mut model = ColumnModel::new();
    model.push(Column::new(100.0));

    let mut table = ColumnGeometryTable::new();
    table.resolve(&model, 100.0, 1, 1.0);

    // Should not be stale for same epoch
    assert!(!table.is_stale(1));

    // Should be stale for different epoch
    assert!(table.is_stale(2));
}

// Note: The internal phase transition tests (enter_layout, enter_paint, etc.)
// are enforced by TabularSubstrate's Widget implementation. The Widget trait
// methods (layout, paint, handle_event) internally use the guards, but
// the guard state itself is not directly testable through public APIs.
//
// The observable behavior is that:
// 1. Geometry is resolved during layout (verified by geometry_resolution tests)
// 2. Geometry is read-only during paint (enforced internally)
// 3. Geometry is read-only during event handling (enforced internally)
//
// These behaviors are tested implicitly through the full widget lifecycle
// in integration tests that exercise TabularSubstrate.
