//! SPEC-HAL-081A/081C ABI Guard
//!
//! This module provides compile-time enforcement that compositor
//! structs remain consistent. Any structural change to SurfaceDescriptor or
//! SurfaceState will cause compilation or test failures.
//!
//! # Enforcement Mechanism
//!
//! 1. Exhaustive pattern matching on struct fields (compile-time)
//! 2. Size assertions to detect hidden fields (test-time)
//! 3. Field count validation via destructuring (compile-time)
//!
//! # How to Trigger Failure
//!
//! Adding a field to SurfaceDescriptor or SurfaceState will cause:
//! - `error[E0027]: pattern does not mention field `new_field``
//!
//! This is mechanical enforcement, not review-based.
//!
//! # SPEC-HAL-081C Extension
//!
//! SurfaceDescriptor: UNCHANGED (ABI frozen, no new runtime fields)
//! SurfaceState: Added parent_surface for overlay provenance (internal only)

#[cfg(test)]
mod tests {
    // Module-relative imports - no visibility widening required
    use super::super::surface::{SurfaceDescriptor, SurfaceState, SurfaceVisibility};
    use super::super::{Compositor, SceneKey};
    use crate::core::math::RectF;

    /// ABI GUARD: SurfaceDescriptor field set.
    ///
    /// This test uses exhaustive destructuring. If ANY field is added to
    /// SurfaceDescriptor, this test will FAIL TO COMPILE with:
    /// `error[E0027]: pattern does not mention field `<new_field>``
    ///
    /// Allowed fields (SPEC-HAL-081A + SPEC-HAL-081B):
    /// - id, z_index, bounds, clip, visibility, opacity, scene_key
    /// - _role (PhantomData, type-level only, zero runtime size)
    ///
    /// SPEC-HAL-081C: SurfaceDescriptor is UNCHANGED (ABI frozen).
    /// Overlay provenance exists only in compositor-internal SurfaceState.
    #[test]
    fn surface_descriptor_abi_guard() {
        // Obtain SurfaceId through Compositor (no visibility widening)
        let mut compositor = Compositor::new(RectF::new(0.0, 0.0, 800.0, 600.0));
        let surface_id =
            compositor.create_surface(0, RectF::new(0.0, 0.0, 100.0, 100.0), SceneKey::BASE);
        let descriptor = compositor.get_surface(surface_id).unwrap();

        // Exhaustive destructuring - compile fails if fields added/removed
        let SurfaceDescriptor {
            id: _,
            z_index: _,
            bounds: _,
            clip: _,
            visibility: _,
            opacity: _,
            scene_key: _,
            _role: _,
        } = descriptor;

        // Size assertion as secondary guard
        let size = std::mem::size_of::<SurfaceDescriptor>();
        assert!(
            size <= 128,
            "SurfaceDescriptor size changed unexpectedly: {} bytes. ABI violation?",
            size
        );
    }

    /// ABI GUARD: SurfaceState field set.
    ///
    /// Same mechanism as SurfaceDescriptor guard.
    /// Uses module-relative access to SurfaceState (pub(super) visibility).
    ///
    /// Allowed fields (SPEC-HAL-081A + SPEC-HAL-081C):
    /// - id, z_index, bounds, clip, visibility, opacity, scene_key
    /// - parent_surface (SPEC-HAL-081C: overlay provenance, internal only)
    ///
    /// Note: Overlay-ness is structurally inferred via parent_surface.is_some().
    /// No runtime role enum - role is determined by construction path.
    #[test]
    fn surface_state_abi_guard() {
        // Obtain SurfaceId through Compositor
        let mut compositor = Compositor::new(RectF::new(0.0, 0.0, 800.0, 600.0));
        let surface_id =
            compositor.create_surface(0, RectF::new(0.0, 0.0, 100.0, 100.0), SceneKey::BASE);

        // Create SurfaceState via module-relative access (pub(super) is accessible)
        let state = SurfaceState::new(
            surface_id,
            0,
            RectF::new(0.0, 0.0, 100.0, 100.0),
            SceneKey::BASE.get(),
        );

        // Exhaustive destructuring - compile fails if fields added/removed
        let SurfaceState {
            id: _,
            z_index: _,
            bounds: _,
            clip: _,
            visibility: _,
            opacity: _,
            scene_key: _,
            parent_surface: _,
        } = state;

        // Size assertion as secondary guard (increased for parent_surface)
        let size = std::mem::size_of::<SurfaceState>();
        assert!(
            size <= 128,
            "SurfaceState size changed unexpectedly: {} bytes. ABI violation?",
            size
        );
    }

    /// Verify SurfaceDescriptor and SurfaceState have consistent field mapping.
    ///
    /// Invariant: SurfaceState.to_descriptor() must produce consistent values.
    ///
    /// SPEC-HAL-081C: SurfaceState.parent_surface is NOT in SurfaceDescriptor.
    /// Overlay provenance exists only in compositor-internal state.
    /// SurfaceDescriptor is renderer-facing and remains ABI frozen.
    #[test]
    fn descriptor_state_field_parity() {
        let mut compositor = Compositor::new(RectF::new(0.0, 0.0, 800.0, 600.0));
        let surface_id =
            compositor.create_surface(10, RectF::new(1.0, 2.0, 3.0, 4.0), SceneKey::BASE);

        let state = SurfaceState::new(
            surface_id,
            10,
            RectF::new(1.0, 2.0, 3.0, 4.0),
            SceneKey::BASE.get(),
        );
        let descriptor = state.to_descriptor();

        // If fields diverge, this comparison will fail or not compile
        assert_eq!(state.id, descriptor.id);
        assert_eq!(state.z_index, descriptor.z_index);
        assert_eq!(state.bounds, descriptor.bounds);
        assert_eq!(state.clip, descriptor.clip);
        assert_eq!(state.visibility, descriptor.visibility);
        assert_eq!(state.opacity, descriptor.opacity);
        assert_eq!(state.scene_key, descriptor.scene_key);

        // Note: state.parent_surface is NOT compared - it's compositor-internal only.
        // SurfaceDescriptor does not expose overlay provenance (ABI frozen).
    }
}
