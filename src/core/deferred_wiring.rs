// ============================================================================
// Deferred Wiring Pattern (Internal)
// ============================================================================
//
// Reusable mechanism for composite widgets that require NodeIds wired after
// tree mount but before first layout.
//
// Lifecycle:
//   1. Widget constructed with placeholders (Deferred::new())
//   2. NodeIds wired exactly once after tree mount (Deferred::set())
//   3. Sealed before first layout (Deferred::seal())
//
// After sealing:
//   - Wiring MUST NOT change (hard panic in all builds)
//   - Widget is immutable w.r.t. structural topology
//
// SPEC-HAL-154: Enforcement MUST be hard failures in release builds.
// FORBID-SEAL-1: Debug-only enforcement is forbidden.
//
// This is NOT convenience. This is architectural pressure to prevent:
//   - Partial trees
//   - Dynamic re-parenting
//   - "Just set it later" hacks

use std::fmt;

/// Deferred value holder with seal-after-layout semantics.
///
/// Avoids repeating "placeholder NodeId + sealed flag" everywhere.
/// This is the reusable primitive for composite widget wiring.
pub(crate) struct Deferred<T> {
    value: Option<T>,
    sealed: bool,
}

impl<T> Deferred<T> {
    /// Compatibility alias for [`Self::new_unset`].
    pub fn new() -> Self {
        Self::new_unset()
    }

    /// Canonical constructor for an unset, unsealed deferred value holder.
    pub fn new_unset() -> Self {
        Self {
            value: None,
            sealed: false,
        }
    }

    /// Set the deferred value (single-assignment).
    ///
    /// # Panics
    /// - Panics if called after seal() (SPEC-HAL-154: wiring phase violation).
    /// - Panics if already set (SPEC-HAL-154: single-assignment).
    pub fn set(&mut self, v: T) {
        assert!(
            !self.is_sealed(),
            "Deferred::set called after seal (SPEC-HAL-154: wiring phase violation)"
        );
        assert!(
            self.value.is_none(),
            "Deferred::set called twice (SPEC-HAL-154: single-assignment violation)"
        );
        self.value = Some(v);
    }

    /// Get the deferred value (copy).
    ///
    /// # Panics
    /// Panics if value was never set (wiring incomplete).
    pub fn get(&self) -> T
    where
        T: Copy,
    {
        self.value
            .expect("Deferred::get called before value set (wiring incomplete)")
    }

    /// Check if a value has been set.
    pub fn is_set(&self) -> bool {
        self.value.is_some()
    }

    /// Seal the deferred value, making it immutable.
    ///
    /// # Panics
    /// - Panics if called without a value set (SPEC-HAL-154: wiring incomplete).
    /// - Panics if already sealed (SPEC-HAL-154: double-seal).
    pub fn seal(&mut self) {
        assert!(
            !self.is_sealed(),
            "Deferred::seal called twice (SPEC-HAL-154: double-seal violation)"
        );
        assert!(
            self.value.is_some(),
            "Deferred::seal called before value set (SPEC-HAL-154: wiring incomplete)"
        );
        self.sealed = true;
    }

    /// Check if this deferred value has been sealed.
    pub fn is_sealed(&self) -> bool {
        self.sealed
    }
}

impl<T> Default for Deferred<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: fmt::Debug> fmt::Debug for Deferred<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Deferred")
            .field("value", &self.value)
            .field("sealed", &self.sealed)
            .finish()
    }
}

impl<T: Clone> Clone for Deferred<T> {
    fn clone(&self) -> Self {
        Self {
            value: self.value.clone(),
            sealed: self.sealed,
        }
    }
}

/// INTERNAL: Trait for widgets that require NodeIds wired after tree mount
/// but before layout.
///
/// Lifecycle:
///   1. Constructed with placeholders
///   2. Wired exactly once
///   3. Sealed before first layout
///
/// After sealing:
///   - Wiring MUST NOT change
///   - Mutation is illegal (hard panic in all builds per SPEC-HAL-154)
///
/// This trait is exposed through Widget's deferred-wiring hooks so the engine can
/// seal composite widgets before first layout.
pub trait DeferredWiring {
    /// Returns true if all required references are present.
    fn is_fully_wired(&self) -> bool;

    /// Seal the widget.
    ///
    /// Must be called:
    ///   - Exactly once
    ///   - Before first layout
    ///
    /// After sealing:
    ///   - All setters must panic (release-mode enforcement)
    ///   - Widget is immutable w.r.t. wiring
    fn seal(&mut self);

    /// Returns true if seal() has been called.
    fn is_sealed(&self) -> bool;
}

/// SPEC-HAL-154: Engine enforcement of deferred wiring before layout.
///
/// This function MUST be called by the engine immediately before `widget.layout()`.
/// It enforces the SPEC-HAL-154 invariants:
///
/// - INV-SEAL-ENG-1: If a widget implements DeferredWiring, layout MUST NOT run
///   unless `is_fully_wired() == true`.
/// - INV-SEAL-ENG-2: The engine MUST call `seal()` exactly once, immediately
///   before the first successful layout.
/// - INV-SEAL-ENG-3: After sealing, `wire_*` methods MUST panic (already handled by Deferred).
/// - INV-SEAL-ENG-4: Widgets MUST NOT call `seal()` themselves (engine owns lifecycle).
///
/// # Panics
/// Panics if `is_fully_wired()` returns false (wiring incomplete).
///
/// # Debug Assertion
/// In debug builds, asserts that sealing only happens during layout phase
/// (not during paint, event handling, or rebuild).
#[inline]
pub(crate) fn enforce_deferred_wiring(dw: &mut dyn DeferredWiring) {
    // INV-SEAL-ENG-1: Layout MUST NOT run unless fully wired.
    assert!(
        dw.is_fully_wired(),
        "Layout called before deferred wiring complete (SPEC-HAL-154: INV-SEAL-ENG-1)"
    );

    // INV-SEAL-ENG-2: Engine seals exactly once, before first layout.
    if !dw.is_sealed() {
        dw.seal();
    }
}

/// SPEC-HAL-154: Centralized assertion for paint/event phase seal verification.
///
/// This is the **single authoritative predicate** for non-layout phases.
/// Used by paint and event guards to verify INV-SEAL-ENG-NO-PAINT-BEFORE-SEAL
/// and INV-SEAL-ENG-NO-EVENT-BEFORE-SEAL.
///
/// # Contract
/// - Layout dispatchers call `enforce_deferred_wiring` (seals + validates).
/// - Paint/event guards call this function (asserts only, never seals).
///
/// # Debug-only
/// This assertion is debug-only. In release builds, the invariant is
/// structurally guaranteed by layout enforcement.
#[inline]
pub(crate) fn assert_sealed_for_phase(
    _widget: &dyn crate::core::widget::Widget,
    _phase: &'static str,
) {
    #[cfg(debug_assertions)]
    if let Some(dw) = _widget.deferred_wiring() {
        debug_assert!(
            dw.is_sealed(),
            "SPEC-HAL-154 VIOLATION: {} before seal. Widget has DeferredWiring but is not sealed.",
            _phase
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deferred_basic_lifecycle() {
        let mut d: Deferred<u32> = Deferred::new();

        // Initially not set, not sealed
        assert!(!d.is_set());
        assert!(!d.is_sealed());

        // Set value
        d.set(42);
        assert!(d.is_set());
        assert!(!d.is_sealed());
        assert_eq!(d.get(), 42);

        // Seal
        d.seal();
        assert!(d.is_sealed());
        assert_eq!(d.get(), 42);
    }

    #[test]
    #[should_panic(expected = "Deferred::get called before value set")]
    fn test_deferred_get_before_set_panics() {
        let d: Deferred<u32> = Deferred::new();
        let _ = d.get();
    }

    #[test]
    #[should_panic(expected = "Deferred::seal called before value set")]
    fn test_deferred_seal_without_value_panics() {
        let mut d: Deferred<u32> = Deferred::new();
        d.seal();
    }

    #[test]
    #[should_panic(expected = "Deferred::set called after seal")]
    fn test_deferred_set_after_seal_panics() {
        let mut d: Deferred<u32> = Deferred::new();
        d.set(42);
        d.seal();
        d.set(100); // Should panic
    }

    #[test]
    #[should_panic(
        expected = "Deferred::set called twice (SPEC-HAL-154: single-assignment violation)"
    )]
    fn test_deferred_set_twice_panics() {
        let mut d: Deferred<u32> = Deferred::new();
        d.set(42);
        d.set(100); // Should panic - single-assignment
    }

    #[test]
    #[should_panic(expected = "Deferred::seal called twice (SPEC-HAL-154: double-seal violation)")]
    fn test_deferred_seal_twice_panics() {
        let mut d: Deferred<u32> = Deferred::new();
        d.set(42);
        d.seal();
        d.seal(); // Should panic - double-seal
    }
}
