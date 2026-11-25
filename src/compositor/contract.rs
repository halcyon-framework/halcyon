//! SPEC-HAL-081A/081B: Compositor Contract Marker Types
//!
//! These marker types encode the compositor's contract at the type level.
//! They provide compile-time guarantees for architectural boundaries.
//!
//! # Contract System
//!
//! This module defines the **sealed contract system** for the compositor.
//! The `Contract` trait is sealed via `private::Sealed` - external crates
//! cannot implement it.
//!
//! Available contracts:
//! - `BaseContract` (SPEC-HAL-081A): Single base surface, current default
//! - `MultiSurfaceContract` (SPEC-HAL-081B): Multiple base surfaces permitted
//!
//! # Design Rationale
//!
//! The type system enforces:
//! - No silent architectural drift
//! - Contract changes require explicit type changes
//! - External crates cannot invent new contracts (sealed trait)
//!
//! # Usage
//!
//! ```ignore
//! // Current compositor contract:
//! let snapshot: SurfaceStackSnapshot<BaseContract> = compositor.snapshot();
//! ```

use std::marker::PhantomData;
use std::num::NonZeroU64;

// ============================================================================
// SceneKey — Explicit, Non-Zero Surface Identifier
// ============================================================================

/// Scene key identifying which compositor surface content belongs to.
///
/// # Contract Law
///
/// - SceneKey is ALWAYS non-zero. Zero is not a valid scene key.
/// - SceneKey must be explicitly provided at construction - no implicit defaults.
/// - Renderer validates non-zero at execution boundary.
///
/// # Usage
///
/// ```ignore
/// // Create a scene key (panics if zero)
/// let key = SceneKey::new(1);
///
/// // Use the constant for base surface
/// compositor.set_surface_scene_key(surface_id, SceneKey::BASE);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SceneKey(NonZeroU64);

impl SceneKey {
    /// Base surface scene key (value = 1).
    ///
    /// Current contract uses a single base surface with this key.
    pub const BASE: SceneKey = SceneKey(
        // SAFETY: 1 is non-zero
        unsafe { NonZeroU64::new_unchecked(1) },
    );

    /// Create a new SceneKey from a raw value.
    ///
    /// # Panics
    ///
    /// Panics if `value` is zero.
    #[inline]
    pub fn new(value: u64) -> Self {
        Self(NonZeroU64::new(value).expect("SceneKey cannot be zero"))
    }

    /// Try to create a SceneKey, returning None if value is zero.
    #[inline]
    pub fn try_new(value: u64) -> Option<Self> {
        NonZeroU64::new(value).map(Self)
    }

    /// Get the raw u64 value.
    #[inline]
    pub const fn get(self) -> u64 {
        self.0.get()
    }

    /// Check if a raw u64 is a valid scene key (non-zero).
    #[inline]
    pub const fn is_valid(value: u64) -> bool {
        value != 0
    }
}

impl std::fmt::Display for SceneKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ============================================================================
// Phase Marker Traits
// ============================================================================

/// Sealed trait for compositor contract markers.
///
/// This trait is sealed to prevent external implementation.
pub trait Contract: private::Sealed {
    /// Human-readable name for debug output.
    const NAME: &'static str;

    /// Maximum number of surfaces allowed under this contract.
    const MAX_SURFACES: usize;
}

mod private {
    pub trait Sealed {}
    impl Sealed for super::BaseContract {}
    impl Sealed for super::MultiSurfaceContract {}
}

// ============================================================================
// Current Compositor Contract Marker
// ============================================================================

/// Marker type for the current compositor contract (SPEC-HAL-081A).
///
/// # Current Contract Constraints
///
/// - Single surface (base surface)
/// - O(S×B) batch filtering (acceptable for single surface)
/// - All content routes through the base surface
/// - No overlay, modal, or multi-surface composition
///
/// # Compile-Time Guarantees
///
/// Types parameterized with `BaseContract` cannot:
/// - Create multiple surfaces (enforced at type boundary)
/// - Use multi-surface routing APIs (they don't exist)
/// - Accidentally drift into unsupported behavior
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BaseContract;

impl Contract for BaseContract {
    const NAME: &'static str = "current-contract";
    const MAX_SURFACES: usize = 1;
}

// ============================================================================
// SPEC-HAL-081B Multi-Surface Contract
// ============================================================================

/// Multi-surface contract marker (SPEC-HAL-081B).
///
/// # Contract Constraints
///
/// - Multiple base surfaces permitted
/// - Overlay rules unchanged (renderer-invisible, cascade destruction, z-order)
/// - No upper bound on surface count (practical limit is memory)
///
/// # Compile-Time Guarantees
///
/// Types parameterized with `MultiSurfaceContract`:
/// - May create multiple base surfaces
/// - Still enforce overlay invariants
/// - Still enforce authority boundaries (INV-COMP-N*)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MultiSurfaceContract;

impl Contract for MultiSurfaceContract {
    const NAME: &'static str = "multi-surface-contract";
    const MAX_SURFACES: usize = usize::MAX;
}

// ============================================================================
// Contract Enforcement
// ============================================================================

/// Compile-time assertion for current compositor contract.
///
/// Use this in function bounds to restrict APIs to the current contract:
///
/// ```ignore
/// fn current_contract_only<P: AssertBaseContract>() { ... }
/// ```
pub trait AssertBaseContract: Contract {}
impl AssertBaseContract for BaseContract {}

/// Compile-time assertion for multi-surface compositor contract.
///
/// Use this in function bounds to restrict APIs to multi-surface contract:
///
/// ```ignore
/// fn multi_surface_only<P: AssertMultiSurfaceContract>() { ... }
/// ```
pub trait AssertMultiSurfaceContract: Contract {}
impl AssertMultiSurfaceContract for MultiSurfaceContract {}

/// Runtime assertion for single-surface invariant.
///
/// # Panics
///
/// Panics if surface_count > 1 (in BOTH debug and release builds).
///
/// # Contract Enforcement
///
/// This is a hard enforcement point for SPEC-HAL-081A. The single-surface
/// invariant is architectural, not a debugging aid. Violating it in release
/// builds would cause undefined rendering behavior.
#[inline]
pub fn assert_base_contract_surface_count(surface_count: usize) {
    assert!(
        surface_count <= BaseContract::MAX_SURFACES,
        "SPEC-HAL-081A violation: {} surfaces created, but current contract allows at most {}. \
         Multi-surface composition is not supported under current contract.",
        surface_count,
        BaseContract::MAX_SURFACES
    );
}

// ============================================================================
// Phase-Parameterized Wrapper (for downstream use)
// ============================================================================

/// A wrapper that carries contract information at the type level.
///
/// This is a zero-cost abstraction - PhantomData adds no runtime overhead.
/// The contract marker is purely compile-time information.
///
/// # Usage
///
/// Downstream types can use this to inherit contract semantics:
///
/// ```ignore
/// pub struct CompiledDrawPlan<P: Phase> {
///     _base_contract: BaseContract<P>,
///     // ... other fields
/// }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Contracted<P: Contract> {
    _marker: PhantomData<P>,
}

impl<P: Contract> Contracted<P> {
    /// Create a new contract marker.
    #[inline]
    pub const fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }

    /// Get the contract name.
    #[inline]
    pub const fn phase_name(&self) -> &'static str {
        P::NAME
    }

    /// Get the maximum allowed surfaces for this contract.
    #[inline]
    pub const fn max_surfaces(&self) -> usize {
        P::MAX_SURFACES
    }
}

impl<P: Contract> Default for Contracted<P> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_contract_constants() {
        assert_eq!(BaseContract::NAME, "current-contract");
        assert_eq!(BaseContract::MAX_SURFACES, 1);
    }

    #[test]
    fn phased_marker() {
        let marker: Contracted<BaseContract> = Contracted::new();
        assert_eq!(marker.phase_name(), "current-contract");
        assert_eq!(marker.max_surfaces(), 1);
    }

    #[test]
    fn assert_single_surface() {
        // Should not panic
        assert_base_contract_surface_count(0);
        assert_base_contract_surface_count(1);
    }

    #[test]
    #[should_panic(expected = "SPEC-HAL-081A violation")]
    fn assert_rejects_multiple_surfaces() {
        assert_base_contract_surface_count(2);
    }

    // ========================================================================
    // SPEC-HAL-081A Freeze Guards
    // ========================================================================

    /// Guard: BaseContract implements Contract.
    ///
    /// This test verifies that BaseContract can be used where Contract is required.
    /// If this test compiles, the sealed trait pattern is working correctly.
    #[test]
    fn guard_base_contract_implements_contract() {
        fn requires_contract<P: Contract>() -> &'static str {
            P::NAME
        }

        // BaseContract implements Contract
        assert_eq!(requires_contract::<BaseContract>(), "current-contract");
    }

    /// Guard: MultiSurfaceContract implements Contract (SPEC-HAL-081B).
    ///
    /// This test verifies that MultiSurfaceContract is sealed and implements Contract.
    #[test]
    fn guard_multi_surface_contract_implements_contract() {
        fn requires_contract<P: Contract>() -> &'static str {
            P::NAME
        }

        // MultiSurfaceContract implements Contract
        assert_eq!(
            requires_contract::<MultiSurfaceContract>(),
            "multi-surface-contract"
        );
    }

    /// Guard: MultiSurfaceContract has correct MAX_SURFACES.
    #[test]
    fn guard_multi_surface_contract_allows_multiple() {
        assert_eq!(MultiSurfaceContract::MAX_SURFACES, usize::MAX);
        assert!(MultiSurfaceContract::MAX_SURFACES > 1);
    }

    /// Guard: Contract trait is sealed (external impl impossible).
    ///
    /// The private::Sealed supertrait prevents external Contract implementations.
    /// This test documents the sealing mechanism.
    #[test]
    fn guard_contract_trait_is_sealed() {
        // The Contract trait has a Sealed supertrait from private module.
        // External crates cannot implement Sealed, therefore cannot implement Contract.
        // This is enforced at compile-time by the trait definition.
        //
        // Evidence: Contract: private::Sealed bound in trait definition
        //
        // Both BaseContract and MultiSurfaceContract implement Sealed in the
        // private module, making them the only valid Contract implementors.
    }

    /// Guard: AssertMultiSurfaceContract trait works.
    #[test]
    fn guard_assert_multi_surface_contract_trait() {
        fn requires_multi_surface<P: AssertMultiSurfaceContract>() -> &'static str {
            P::NAME
        }

        assert_eq!(
            requires_multi_surface::<MultiSurfaceContract>(),
            "multi-surface-contract"
        );

        // The following would fail to compile (BaseContract doesn't implement AssertMultiSurfaceContract):
        // requires_multi_surface::<BaseContract>();
    }
}
