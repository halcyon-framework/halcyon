// =============================================================================
// SPEC-HAL-164/165/168: TabularSubstrate Invariant Enforcement Guards
// =============================================================================
//
// This module provides guards that enforce TabularSubstrate invariants:
//
// SPEC-HAL-164:
// - INVARIANT-1: Column geometry resolved once per evaluation.
// - INVARIANT-2: Header/Body/Footer must not compute column widths independently.
// - INVARIANT-3: No projection may mutate column geometry during paint or input.
//
// SPEC-HAL-165:
// - INVARIANT-1: Column Model owned exclusively by TabularSubstrate.
// - INVARIANT-4: Column geometry resolved at most once per evaluation.
// - INVARIANT-5: Cannot read geometry before resolution.
// - INVARIANT-6: Cannot re-resolve geometry after resolution.
//
// SPEC-HAL-168:
// - INVARIANT-1: Horizontal geometry resolution owned by TabularSubstrate layout phase.
// - INVARIANT-4: Horizontal geometry resolved exactly once per layout pass.
// =============================================================================

use std::cell::Cell;

/// Frame phase for tracking which operations are legal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FramePhase {
    /// Before layout has started.
    PreLayout,
    /// During layout phase - geometry can be resolved/read.
    Layout,
    /// After layout, during paint - geometry read-only.
    Paint,
    /// During event dispatch - geometry read-only.
    Event,
}

/// Guard for tracking evaluation phase and enforcing phase-specific invariants.
///
/// SPEC-HAL-164/165/168: Ensures geometry operations happen in the correct phase.
#[derive(Debug)]
pub struct EvaluationPhaseGuard {
    /// Current evaluation phase.
    phase: Cell<FramePhase>,
    /// Current evaluation epoch.
    evaluation_epoch: Cell<u64>,
    /// Whether geometry has been resolved this evaluation.
    geometry_resolved: Cell<bool>,
}

impl EvaluationPhaseGuard {
    /// Create a new guard in PreLayout phase.
    pub fn new() -> Self {
        Self {
            phase: Cell::new(FramePhase::PreLayout),
            evaluation_epoch: Cell::new(0),
            geometry_resolved: Cell::new(false),
        }
    }

    /// Get current evaluation phase.
    pub fn phase(&self) -> FramePhase {
        self.phase.get()
    }

    /// Get current evaluation epoch.
    pub fn evaluation_epoch(&self) -> u64 {
        self.evaluation_epoch.get()
    }

    /// Check if geometry has been resolved this evaluation.
    pub fn is_geometry_resolved(&self) -> bool {
        self.geometry_resolved.get()
    }

    /// Begin a new evaluation.
    ///
    /// Resets phase to PreLayout and increments evaluation epoch.
    pub fn begin_evaluation(&self) {
        self.evaluation_epoch.set(self.evaluation_epoch.get() + 1);
        self.phase.set(FramePhase::PreLayout);
        self.geometry_resolved.set(false);
    }

    /// Enter layout phase.
    ///
    /// SPEC-HAL-168 INVARIANT-1: Only TabularSubstrate layout phase may resolve geometry.
    pub fn enter_layout(&self) {
        debug_assert_eq!(
            self.phase.get(),
            FramePhase::PreLayout,
            "SPEC-HAL-168: Layout phase must be entered from PreLayout"
        );
        self.phase.set(FramePhase::Layout);
    }

    /// Mark geometry as resolved.
    ///
    /// SPEC-HAL-165 INVARIANT-4: Geometry resolved at most once per evaluation.
    pub fn mark_geometry_resolved(&self) {
        if self.geometry_resolved.get() {
            panic!(
                "\n\
                ╔══════════════════════════════════════════════════════════════════════╗\n\
                ║  SPEC-HAL-165 INVARIANT-4 VIOLATION: Double geometry resolution       ║\n\
                ╠══════════════════════════════════════════════════════════════════════╣\n\
                ║  Column geometry was already resolved for evaluation {}.            \n\
                ║  Geometry must be resolved at most once per evaluation.              ║\n\
                ╚══════════════════════════════════════════════════════════════════════╝",
                self.evaluation_epoch.get()
            );
        }
        self.geometry_resolved.set(true);
    }

    /// Enter paint phase.
    pub fn enter_paint(&self) {
        debug_assert!(
            self.phase.get() == FramePhase::Layout || self.phase.get() == FramePhase::PreLayout,
            "SPEC-HAL-164: Paint phase must follow Layout phase"
        );
        self.phase.set(FramePhase::Paint);
    }

    /// Enter event phase.
    pub fn enter_event(&self) {
        self.phase.set(FramePhase::Event);
    }

    /// Assert that geometry can be read.
    ///
    /// SPEC-HAL-165 INVARIANT-5: Cannot read geometry before resolution.
    pub fn assert_geometry_readable(&self) {
        if !self.geometry_resolved.get() {
            panic!(
                "\n\
                ╔══════════════════════════════════════════════════════════════════════╗\n\
                ║  SPEC-HAL-165 INVARIANT-5 VIOLATION: Geometry read before resolution  ║\n\
                ╠══════════════════════════════════════════════════════════════════════╣\n\
                ║  Column geometry was accessed before resolve() was called.           ║\n\
                ║  Geometry must be resolved during layout phase before any reads.     ║\n\
                ╚══════════════════════════════════════════════════════════════════════╝"
            );
        }
    }

    /// Assert that geometry can be mutated.
    ///
    /// SPEC-HAL-164 INVARIANT-3: No projection may mutate geometry during paint or input.
    pub fn assert_geometry_mutable(&self) {
        let phase = self.phase.get();
        if phase != FramePhase::Layout && phase != FramePhase::PreLayout {
            panic!(
                "\n\
                ╔══════════════════════════════════════════════════════════════════════╗\n\
                ║  SPEC-HAL-164 INVARIANT-3 VIOLATION: Geometry mutation outside layout ║\n\
                ╠══════════════════════════════════════════════════════════════════════╣\n\
                ║  Attempted to mutate column geometry during {:?} phase.            \n\
                ║  Geometry may only be mutated during Layout phase.                   ║\n\
                ╚══════════════════════════════════════════════════════════════════════╝",
                phase
            );
        }
    }

    /// Assert that we are in layout phase.
    ///
    /// SPEC-HAL-168 INVARIANT-1: Geometry resolution owned by layout phase.
    pub fn assert_layout_phase(&self) {
        if self.phase.get() != FramePhase::Layout {
            panic!(
                "\n\
                ╔══════════════════════════════════════════════════════════════════════╗\n\
                ║  SPEC-HAL-168 INVARIANT-1 VIOLATION: Operation outside layout phase   ║\n\
                ╠══════════════════════════════════════════════════════════════════════╣\n\
                ║  This operation is only legal during the Layout phase.               ║\n\
                ║  Current phase: {:?}                                                \n\
                ╚══════════════════════════════════════════════════════════════════════╝",
                self.phase.get()
            );
        }
    }
}

impl Default for EvaluationPhaseGuard {
    fn default() -> Self {
        Self::new()
    }
}

/// Guard for ensuring column model ownership.
///
/// SPEC-HAL-165 INVARIANT-1: Column Model owned exclusively by TabularSubstrate.
/// SPEC-HAL-165 INVARIANT-2: Projections may observe but not mutate.
#[derive(Debug)]
pub struct ColumnModelGuard {
    /// Whether mutation is currently allowed.
    mutation_allowed: Cell<bool>,
}

impl ColumnModelGuard {
    /// Create a new guard with mutation allowed.
    pub fn new() -> Self {
        Self {
            mutation_allowed: Cell::new(true),
        }
    }

    /// Lock the model to prevent mutation (called before projections run).
    pub fn lock(&self) {
        self.mutation_allowed.set(false);
    }

    /// Unlock the model to allow mutation (called after projections complete).
    pub fn unlock(&self) {
        self.mutation_allowed.set(true);
    }

    /// Assert that mutation is allowed.
    ///
    /// SPEC-HAL-165 INVARIANT-2: Projections may observe but not mutate.
    pub fn assert_mutation_allowed(&self) {
        if !self.mutation_allowed.get() {
            panic!(
                "\n\
                ╔══════════════════════════════════════════════════════════════════════╗\n\
                ║  SPEC-HAL-165 INVARIANT-2 VIOLATION: Column model mutation from proj  ║\n\
                ╠══════════════════════════════════════════════════════════════════════╣\n\
                ║  A projection attempted to mutate the column model.                  ║\n\
                ║  Projections may observe column state but must not mutate it.        ║\n\
                ╚══════════════════════════════════════════════════════════════════════╝"
            );
        }
    }

    /// Check if mutation is allowed (without panicking).
    pub fn is_mutation_allowed(&self) -> bool {
        self.mutation_allowed.get()
    }
}

impl Default for ColumnModelGuard {
    fn default() -> Self {
        Self::new()
    }
}

/// Guard for ensuring row model ownership.
///
/// SPEC-HAL-167 INVARIANT-4: Only virtualization layer decides which rows are realized.
#[derive(Debug)]
pub struct RowModelGuard {
    /// Whether realization changes are allowed.
    realization_allowed: Cell<bool>,
}

impl RowModelGuard {
    /// Create a new guard with realization allowed.
    pub fn new() -> Self {
        Self {
            realization_allowed: Cell::new(true),
        }
    }

    /// Lock row realization (called during non-virtualization phases).
    pub fn lock(&self) {
        self.realization_allowed.set(false);
    }

    /// Unlock row realization (called when virtualization layer is active).
    pub fn unlock(&self) {
        self.realization_allowed.set(true);
    }

    /// Assert that realization changes are allowed.
    ///
    /// SPEC-HAL-167 INVARIANT-4: Only virtualization layer decides realization.
    pub fn assert_realization_allowed(&self) {
        if !self.realization_allowed.get() {
            panic!(
                "\n\
                ╔══════════════════════════════════════════════════════════════════════╗\n\
                ║  SPEC-HAL-167 INVARIANT-4 VIOLATION: Row realization outside virt     ║\n\
                ╠══════════════════════════════════════════════════════════════════════╣\n\
                ║  Attempted to change row realization outside virtualization layer.   ║\n\
                ║  Only the virtualization layer may decide which rows are realized.   ║\n\
                ╚══════════════════════════════════════════════════════════════════════╝"
            );
        }
    }
}

impl Default for RowModelGuard {
    fn default() -> Self {
        Self::new()
    }
}

/// Guard for ensuring row identity stability during sort processing.
///
/// SPEC-HAL-171 INVARIANT-SORT-4: Sorting never changes RowId identity mapping
/// for a given underlying data row; it only changes realization order.
///
/// This guard prevents structural mutations to RowModel (push, remove, reorder)
/// during the sort → realization pipeline, ensuring row identity is stable.
#[derive(Debug)]
pub struct SortProcessingGuard {
    /// Whether sort processing is currently active.
    sort_processing: Cell<bool>,
}

impl SortProcessingGuard {
    /// Create a new guard with sort processing inactive.
    pub fn new() -> Self {
        Self {
            sort_processing: Cell::new(false),
        }
    }

    /// Begin sort processing (locks RowModel mutation).
    ///
    /// SPEC-HAL-171: Called before compute_sorted_indices and realization.
    pub fn begin_sort_processing(&self) {
        self.sort_processing.set(true);
    }

    /// End sort processing (unlocks RowModel mutation).
    ///
    /// SPEC-HAL-171: Called after realization is complete.
    pub fn end_sort_processing(&self) {
        self.sort_processing.set(false);
    }

    /// Check if sort processing is active.
    pub fn is_sort_processing(&self) -> bool {
        self.sort_processing.get()
    }

    /// Assert that RowModel mutation is allowed.
    ///
    /// SPEC-HAL-171 INVARIANT-SORT-4: Panics if called during sort processing.
    /// RowModel must not be structurally mutated during the sort → realization pipeline.
    pub fn assert_row_mutation_allowed(&self) {
        if self.sort_processing.get() {
            panic!(
                "\n\
                ╔══════════════════════════════════════════════════════════════════════╗\n\
                ║  SPEC-HAL-171 INVARIANT-SORT-4 VIOLATION: RowModel mutation during    ║\n\
                ║  sort processing                                                      ║\n\
                ╠══════════════════════════════════════════════════════════════════════╣\n\
                ║  Attempted to mutate RowModel during sort → realization pipeline.    ║\n\
                ║  Sorting affects only realization order, never row identity.          ║\n\
                ║  RowModel must not be structurally changed (push, remove, reorder)    ║\n\
                ║  while sort processing is active.                                     ║\n\
                ╚══════════════════════════════════════════════════════════════════════╝"
            );
        }
    }
}

impl Default for SortProcessingGuard {
    fn default() -> Self {
        Self::new()
    }
}
