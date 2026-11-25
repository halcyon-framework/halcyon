// =============================================================================
// SPEC-HAL-189 §5: Dirty Tracking Primitives (Shared)
// =============================================================================
//
// Shared dirty tracking types used by CellHost, RowHost, and future
// widget lifecycle managers (PropertyEditor, Grid).
//
// Extracted from list_view/row_host.rs per SPEC-HAL-260 §17.
// =============================================================================

use bitflags::bitflags;

// =============================================================================
// Update Phase (SPEC-HAL-189 §5 enforcement)
// =============================================================================

/// Update phase for dirty consumption.
///
/// SPEC-HAL-189: Dirty flags are consumed in a specific order, enforced by phase.
/// Each phase clears only its corresponding flag(s).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdatePhase {
    /// Bind phase: re-fetch data and bind to widget.
    ///
    /// Consumes: BINDING
    /// Precondition: GEOMETRY and POLICY must be clean (already processed).
    Bind,

    /// Policy phase: re-evaluate capabilities.
    ///
    /// Consumes: POLICY
    /// Precondition: None (can run independently).
    Policy,

    /// Layout phase: recompute geometry.
    ///
    /// Consumes: GEOMETRY
    /// Precondition: None (can run independently).
    Layout,

    /// Paint phase: repaint visual presentation.
    ///
    /// Consumes: PRESENTATION
    /// Precondition: None (runs last, clears only PRESENTATION).
    Paint,
}

// =============================================================================
// Dirty Outcome (SPEC-HAL-189 §5 enforcement)
// =============================================================================

/// Outcome of dirty consumption.
///
/// SPEC-HAL-189: consume_dirty() returns explicit outcome, not boolean.
/// This allows the caller to know exactly what happened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirtyOutcome {
    /// No work was needed (flag was not set).
    Clean,

    /// Work was performed (flag was set and cleared).
    Consumed,

    /// Work was blocked (precondition not met).
    ///
    /// Contains the blocking flags that must be cleared first.
    Blocked(DirtyFlags),
}

impl DirtyOutcome {
    /// Returns true if work was consumed.
    pub fn was_consumed(&self) -> bool {
        matches!(self, DirtyOutcome::Consumed)
    }

    /// Returns true if clean (no work needed).
    pub fn was_clean(&self) -> bool {
        matches!(self, DirtyOutcome::Clean)
    }

    /// Returns true if blocked.
    pub fn was_blocked(&self) -> bool {
        matches!(self, DirtyOutcome::Blocked(_))
    }
}

// =============================================================================
// Dirty Flags (SPEC-HAL-189 §5)
// =============================================================================

bitflags! {
    /// Dirty flags per SPEC-HAL-189 §5.
    ///
    /// These flags drive widget updates. No additional flags may be introduced
    /// without amending SPEC-HAL-189.
    ///
    /// INV-189-DIRTY-EXHAUSTIVE: This is a closed set.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct DirtyFlags: u8 {
        /// Underlying data value changed.
        ///
        /// Requires: Rebind data to existing widget.
        /// Does NOT: Affect identity or geometry.
        const BINDING = 0b0001;

        /// Permissions / capabilities / editability changed.
        ///
        /// Requires: Capability re-evaluation.
        /// Does NOT: Recreate widgets.
        const POLICY = 0b0010;

        /// Layout-affecting change (row height, column width).
        ///
        /// Requires: Layout recomputation.
        /// Does NOT: Recreate widgets.
        const GEOMETRY = 0b0100;

        /// Purely visual change (theme, style, selection highlight).
        ///
        /// Requires: Repaint only.
        /// Does NOT: Recreate widgets.
        const PRESENTATION = 0b1000;
    }
}

impl DirtyFlags {
    /// Composite dirtiness (all flags).
    ///
    /// SPEC-HAL-189: FullDirty is equivalent to all above flags.
    /// Still does NOT permit recreation.
    pub fn full() -> Self {
        Self::all()
    }
}

#[cfg(test)]
mod tests {
    use super::DirtyFlags;

    #[test]
    fn dirty_flags_are_exactly_the_four_independent_flags() {
        let flags = [
            DirtyFlags::BINDING,
            DirtyFlags::POLICY,
            DirtyFlags::GEOMETRY,
            DirtyFlags::PRESENTATION,
        ];
        let mut union = DirtyFlags::empty();
        for flag in flags {
            assert!(flag.bits().is_power_of_two());
            assert!(!union.intersects(flag));
            union |= flag;
        }
        assert_eq!(DirtyFlags::all(), union);
        assert_eq!(DirtyFlags::full(), union);
    }
}
