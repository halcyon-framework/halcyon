//! SPEC-HAL-081B: Surface role typing and provenance scaffolding.
//!
//! # Type-Level Surface Role Enforcement
//!
//! This module provides compile-time separation between base and overlay surfaces.
//! The `SurfaceRole` trait is sealed - only `BaseSurfaceTag` and `OverlaySurfaceTag`
//! can implement it.
//!
//! # Design
//!
//! Surface role is encoded at the type level via marker types:
//! - `BaseSurfaceTag` - Primary content surfaces
//! - `OverlaySurfaceTag` - Auxiliary surfaces rendered above parent
//!
//! Under `BaseContract` (SPEC-HAL-081A), only `BaseSurfaceTag` surfaces are
//! constructible and visible to the renderer. Overlay support requires
//! `MultiSurfaceContract` (SPEC-HAL-081B).

use super::SurfaceId;

// ============================================================================
// Sealed SurfaceRole Trait
// ============================================================================

/// Sealed trait for surface role markers.
///
/// This trait is sealed - only `BaseSurfaceTag` and `OverlaySurfaceTag`
/// can implement it. External crates cannot create new surface roles.
pub trait SurfaceRole: private::SealedRole + Copy + Clone + Default {
    /// Human-readable name for debug output.
    const NAME: &'static str;
}

mod private {
    pub trait SealedRole {}
    impl SealedRole for super::BaseSurfaceTag {}
    impl SealedRole for super::OverlaySurfaceTag {}
}

// ============================================================================
// Compile-Time Enforcement Marker Types
// ============================================================================

/// Marker type for base surfaces.
///
/// SPEC-HAL-081B: Compile-time enforcement for base surfaces.
/// Under `BaseContract`, only base surfaces are constructible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BaseSurfaceTag;

impl SurfaceRole for BaseSurfaceTag {
    const NAME: &'static str = "base";
}

/// Marker type for overlay surfaces.
///
/// SPEC-HAL-081B: Compile-time enforcement for overlay surfaces.
/// Under `BaseContract`, overlay surfaces are NOT constructible.
/// Requires `MultiSurfaceContract` (not yet implemented).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OverlaySurfaceTag;

impl SurfaceRole for OverlaySurfaceTag {
    const NAME: &'static str = "overlay";
}

// ============================================================================
// Surface Role Classification
// ============================================================================

/// Surface role classification (runtime enum).
///
/// SPEC-HAL-081B: Every surface has exactly one role, immutable for its lifetime.
///
/// # Invariants (INV-SURF-ROLE-*)
///
/// - INV-SURF-ROLE-01: Role is immutable
/// - INV-SURF-ROLE-02: Base surfaces may exist without parent
/// - INV-SURF-ROLE-03: Overlay surfaces MUST have parent
/// - INV-SURF-ROLE-04: Overlays cannot parent other overlays
///
/// # Note
///
/// This enum is for runtime introspection. For compile-time type-level
/// role enforcement, use `BaseSurfaceTag` / `OverlaySurfaceTag` with
/// the `SurfaceRole` trait.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SurfaceRoleKind {
    /// Primary content surface. May have overlay children. Default role.
    #[default]
    Base,
    /// Auxiliary surface rendered above a parent. MUST have parent surface.
    Overlay,
}

// ============================================================================
// Surface Provenance
// ============================================================================

/// Surface provenance metadata for overlay surfaces.
///
/// SPEC-HAL-081B: Tracks parent-child relationships for overlays.
/// This is NOT embedded in SurfaceDescriptor. It will be stored separately
/// or in an overlay-specific extension type when overlay support is implemented.
///
/// # Invariants (INV-PROV-*)
///
/// - INV-PROV-01: Overlay parent_surface MUST reference valid surface
/// - INV-PROV-02: Parent surface MUST be a Base surface
/// - INV-PROV-03: Parent-child relationships are immutable
/// - INV-PROV-07: Compositor exposes provenance but does NOT interpret it
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SurfaceProvenance {
    /// The role of this surface.
    pub role: SurfaceRoleKind,
    /// Parent surface for overlay surfaces. None for base surfaces.
    pub parent_surface: Option<SurfaceId>,
}

impl SurfaceProvenance {
    /// Create provenance for a base surface (no parent).
    pub fn base() -> Self {
        Self {
            role: SurfaceRoleKind::Base,
            parent_surface: None,
        }
    }

    /// Create provenance for an overlay surface with required parent.
    pub fn overlay(parent: SurfaceId) -> Self {
        Self {
            role: SurfaceRoleKind::Overlay,
            parent_surface: Some(parent),
        }
    }

    /// Check if this is a base surface.
    pub fn is_base(&self) -> bool {
        self.role == SurfaceRoleKind::Base
    }

    /// Check if this is an overlay surface.
    pub fn is_overlay(&self) -> bool {
        self.role == SurfaceRoleKind::Overlay
    }
}

impl Default for SurfaceProvenance {
    fn default() -> Self {
        Self::base()
    }
}
